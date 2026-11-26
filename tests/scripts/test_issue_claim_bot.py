"""Offline contract tests: fake server state, real controller and HTTP adapter."""
import copy
import dataclasses
import importlib.util
import io
import json
from pathlib import Path
import sys
import unittest
import urllib.error

spec = importlib.util.spec_from_file_location("claim_bot", Path(__file__).resolve().parents[2] / "scripts/issue_claim_bot.py")
bot = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = bot
spec.loader.exec_module(bot)


def event(body="/claim", number=10, actor="outsider", comment=100):
    return {"action": "created", "repository": {"full_name": bot.REPOSITORY}, "issue": {"number": number},
        "sender": {"id": 42}, "comment": {"id": comment, "body": body, "created_at": "2026-01-01T00:00:00Z",
        "user": {"login": actor, "id": 42, "type": "User"}}}


def initial(**changes):
    return dataclasses.replace(bot.Snapshot(10, "p", "i", "s", "a", tuple(sorted({"Ready":"r", "In progress":"w", "In review":"v", "Done":"d", "Backlog":"b"}.items())),
        True, False, (), "Ready", "Other agent: preserved\r\nSecond owner: untouched"), **changes)


def pull(**changes):
    value = {"number": 123, "html_url": f"https://github.com/{bot.REPOSITORY}/pull/123", "state":"open", "draft":False,
        "user":{"login":"outsider"}, "base":{"ref":"master", "repo":{"full_name":bot.REPOSITORY}}, "body":"Refs #10"}
    value.update(changes)
    return value


class Fake:
    def __init__(self, e=None, state=None):
        self.single_writer_confirmed = True
        self.e = e or event()
        self.state = state or initial()
        self.writes = []
        self.messages = []
        self.stored = []
        self.pr = pull()
        self.on_assign = None
        self.on_snapshot = None
        self.on_field = None
        self.reads = 0

    def comment(self, n):
        c = copy.deepcopy(self.e["comment"])
        c["issue_url"] = f"https://api.github.com/repos/{bot.REPOSITORY}/issues/{self.e['issue']['number']}"
        return c

    def snapshot(self, n):
        self.reads += 1
        if self.on_snapshot:
            self.on_snapshot(self)
        return self.state

    def receipts(self, req):
        return self.stored[:]

    def receipt(self, req, outcome, message, receipt_id=None):
        value = {"request":req.comment, "digest":req.digest, "outcome":outcome}
        if receipt_id is None:
            receipt_id = 1000 + len(self.stored)
            self.stored.append((receipt_id, value))
        else:
            self.stored = [(i, value if i == receipt_id else v) for i, v in self.stored]
        self.messages.append((outcome, message))
        return receipt_id

    def assign(self, n, actor, remove=False):
        self.writes.append(("remove" if remove else "add", actor))
        people = set(self.state.assignees)
        people.discard(actor.casefold()) if remove else people.add(actor.casefold())
        self.state = dataclasses.replace(self.state, assignees=tuple(sorted(people)))
        if self.on_assign:
            self.on_assign(self, remove)

    def field(self, state, field, value):
        self.writes.append((field, value))
        if self.on_field:
            self.on_field(self, field)
        if field == "s":
            status = next(name for name, i in state.options if i == value["singleSelectOptionId"])
            self.state = dataclasses.replace(self.state, status=status)
        else:
            self.state = dataclasses.replace(self.state, agent=value["text"])

    def pull(self, n):
        return self.pr


class Commands(unittest.TestCase):
    def test_outsider_cycle_and_whole_state(self):
        f = Fake()
        for body, comment, status, owners, pr in [("/claim",100,"In progress",("outsider",),None), ("/review #123",101,"In review",("outsider",),123), ("/unclaim",102,"Ready",(),None)]:
            f.e = event(body=body, comment=comment)
            self.assertEqual(bot.run(f.e, f), "success")
            req = bot.authorize(f.e, f)
            agent = bot.with_record(initial().agent, req, "outsider" if owners else None, status, pr)
            self.assertEqual(f.state, initial(assignees=owners, status=status, agent=agent))
            self.assertEqual(f.messages[-1][0], "success")
        self.assertEqual(len(f.stored), 3)

    def test_backlog_contributor_cycle_preserves_other_agent_lines(self):
        f = Fake(state=initial(status="Backlog"))
        for body, comment, status, owners, pr in [("/claim",100,"In progress",("outsider",),None), ("/review #123",101,"In review",("outsider",),123), ("/unclaim",102,"Ready",(),None)]:
            f.e = event(body=body, comment=comment)
            self.assertEqual(bot.run(f.e, f), "success")
            req = bot.authorize(f.e, f)
            agent = bot.with_record(initial().agent, req, "outsider" if owners else None, status, pr)
            self.assertEqual(f.state, initial(assignees=owners, status=status, agent=agent))
            self.assertEqual(f.messages[-1][0], "success")
        self.assertEqual(len(f.stored), 3)

    def test_backlog_claim_recovers_after_assignment_before_status_failure(self):
        f = Fake(state=initial(status="Backlog"))
        def fail_status(api, field):
            if field == "s":
                raise bot.ApiFailure("synthetic status write failure")
        f.on_field = fail_status
        self.assertEqual(bot.run(f.e, f), "error")
        self.assertEqual(f.state, initial(status="Backlog", assignees=("outsider",)))
        f.on_field = None
        f.e = event(comment=101)
        self.assertEqual(bot.run(f.e, f), "success")
        req = bot.authorize(f.e, f)
        self.assertEqual(f.state, initial(assignees=("outsider",), status="In progress",
            agent=bot.with_record(initial().agent, req, "outsider", "In progress", None)))

    def test_claim_refusal_text_distinguishes_review_from_other_states(self):
        for status in ("In review", "Done", "In progress"):
            for owners in ((), ("outsider",)):
                if status == "In progress" and owners:
                    continue
                with self.subTest(status=status, owners=owners):
                    state = initial(status=status, assignees=owners)
                    f = Fake(state=state)
                    self.assertEqual(bot.run(f.e, f), "refused")
                    self.assertEqual((f.state, f.writes), (state, []))
                    text = f.messages[-1][1]
                    if status == "In review":
                        self.assertIn("In review", text)
                    else:
                        self.assertIn("Backlog", text)
                        self.assertIn("Ready", text)
                        self.assertNotIn("review", text)

    def test_refusals_do_not_change_tracking(self):
        for state, body in [(initial(assignees=("someone",)), "/claim"),
            (initial(status="Backlog", assignees=("someone",)),"/claim"), (initial(status="Backlog", assignees=("a","b")),"/claim"), (initial(assignees=("a","b")), "/unclaim"),
            (initial(status="In progress"),"/claim"), (initial(status="Done"),"/claim"), (initial(open=False),"/claim"),
            (initial(locked=True),"/claim"), (initial(),"/review #123"), (initial(status="In progress"),"/unclaim"),
            (initial(assignees=("outsider",), status="In review"),"/claim"), (initial(),"/claim\n$(whoami)"),
            (initial(agent="Claim bot: broken"),"/claim"), (initial(agent="Other: " + "x"*1000),"/claim")]:
            with self.subTest(state=state, body=body):
                f=Fake(event(body),state)
                self.assertEqual(bot.run(f.e,f),"refused")
                self.assertEqual(f.state,state)
                self.assertEqual(f.writes,[])

    def test_review_rejects_bad_prs(self):
        for change in [{"draft":True},{"state":"closed"},{"user":{"login":"other"}}, {"body":"Refs #100"},
            {"body":"another/repo#10"},{"base":{"ref":"other","repo":{"full_name":bot.REPOSITORY}}},
            {"base":{"ref":"master","repo":{"full_name":"someone/else"}}}, {"html_url":"https://evil.test/pull/123"}]:
            with self.subTest(change=change):
                f=Fake(event("/review #123"),initial(assignees=("outsider",),status="In progress")); f.pr=pull(**change)
                self.assertEqual(bot.run(f.e,f),"refused"); self.assertEqual(f.writes,[])

    def test_review_accepts_canonical_urls(self):
        f=Fake(event(f"/review https://github.com/{bot.REPOSITORY}/pull/123"), initial(assignees=("outsider",),status="In progress"))
        f.pr=pull(body=f"Refs https://github.com/{bot.REPOSITORY}/issues/10")
        self.assertEqual(bot.run(f.e,f),"success")
        self.assertEqual(f.state.status,"In review")

    def test_pr_closed_during_tracking_never_acknowledges_review_success(self):
        f=Fake(event("/review #123"),initial(assignees=("outsider",),status="In progress"))
        def close_pr(api,field):api.pr=pull(state="closed")
        f.on_field=close_pr
        self.assertEqual(bot.run(f.e,f),"refused")
        self.assertEqual(f.messages[-1][0],"refused")

    def test_invalid_commands(self):
        for text in ["/review", "/review #0", "/review #123 ; echo hi", "/CLAIM", "/claim @other", "/review https://github.com/other/repo/pull/123"]:
            with self.subTest(text=text), self.assertRaises(bot.Refusal): bot.command(text)

    def test_non_commands_and_bots_ignored(self):
        for change in [lambda e:e.update(action="edited"), lambda e:e["issue"].update(pull_request={}),
            lambda e:e["comment"]["user"].update(type="Bot"), lambda e:e["comment"].update(body="hello"),
            lambda e:e["repository"].update(full_name="someone/else")]:
            e=event(); change(e); f=Fake(e)
            self.assertEqual(bot.run(e,f),"ignored"); self.assertEqual((f.writes,f.messages),([],[]))

    def test_edited_comment_and_forged_identity_fail_before_receipt(self):
        for field,value in [("body","/unclaim"),("issue_url","https://evil.test"),("created_at","bad")]:
            f=Fake(); original=f.comment
            f.comment=lambda n, field=field,value=value:dict(original(n),**{field:value})
            with self.subTest(field=field),self.assertRaises(bot.Refusal):bot.run(f.e,f)
            self.assertEqual((f.writes,f.messages),([],[]))
        e=event();e["sender"]["id"]=99;f=Fake(e)
        with self.assertRaises(bot.Refusal):bot.run(e,f)
        self.assertEqual(f.writes,[])

    def test_replay_after_release_never_reclaims(self):
        f=Fake();old=copy.deepcopy(f.e)
        self.assertEqual(bot.run(old,f),"success")
        f.e=event("/unclaim",comment=102);self.assertEqual(bot.run(f.e,f),"success")
        f.e=old;state=f.state;writes=f.writes[:]
        self.assertEqual(bot.run(old,f),"duplicate")
        self.assertEqual((f.state,f.writes),(state,writes))

    def test_failed_final_ack_keeps_pending_receipt_and_prevents_replay(self):
        f=Fake();receipt=f.receipt
        def fail_ack(req,outcome,message,receipt_id=None):
            if outcome=="success":raise bot.ApiFailure("synthetic reply failure")
            return receipt(req,outcome,message,receipt_id)
        f.receipt=fail_ack
        with self.assertRaises(bot.ApiFailure):bot.run(f.e,f)
        state,writes=f.state,f.writes[:]
        self.assertEqual(f.stored[0][1]["outcome"],"pending")
        self.assertEqual(bot.run(f.e,f),"duplicate")
        self.assertEqual((f.state,f.writes),(state,writes))

    def test_owner_acknowledgement_is_required_before_any_tracking_write(self):
        for field in ["a","s"]:
            f=Fake();f.single_writer_confirmed=False
            def manual_change(api,changed_field):
                if changed_field==field:
                    api.state=dataclasses.replace(api.state,agent=api.state.agent+"\nMaintainer: concurrent note",status="Done")
            f.on_field=manual_change
            with self.subTest(field=field):
                self.assertEqual(bot.run(f.e,f),"error")
                self.assertEqual(f.writes,[])
                self.assertEqual(f.state,initial())
                self.assertEqual(f.stored[0][1]["outcome"],"error")

    def test_configuration_failure_before_assignment_keeps_error_receipt(self):
        f=Fake()
        def fail(api):raise bot.ApiFailure("synthetic configuration denial")
        f.on_snapshot=fail
        self.assertEqual(bot.run(f.e,f),"error")
        self.assertEqual(f.writes,[])
        self.assertEqual(f.stored[0][1]["outcome"],"error")

    def test_pending_error_and_refusal_receipts_consume_request(self):
        for outcome in ["pending","error","refused","success"]:
            f=Fake();req=bot.authorize(f.e,f);f.receipt(req,outcome,"existing")
            self.assertEqual(bot.run(f.e,f),"duplicate");self.assertEqual(f.writes,[])

    def test_newer_receipt_rejects_delayed_request(self):
        f=Fake();req=dataclasses.replace(bot.authorize(f.e,f),comment=102);f.receipt(req,"refused","new")
        self.assertEqual(bot.run(f.e,f),"refused");self.assertEqual(f.writes,[])
        self.assertEqual(f.stored[0][1]["request"],102)

    def test_changed_digest_collision_rejected(self):
        f=Fake();req=dataclasses.replace(bot.authorize(f.e,f),body="/unclaim");f.receipt(req,"success","old")
        with self.assertRaises(bot.Refusal):bot.run(f.e,f)
        self.assertEqual(f.writes,[])

    def test_agent_watermark_refuses_replay_even_without_receipt(self):
        f=Fake();req=bot.authorize(f.e,f);f.state=initial(agent=bot.with_record("",req,None,"Ready",None))
        self.assertEqual(bot.run(f.e,f),"refused");self.assertEqual(f.writes,[])

    def test_drift_before_write_keeps_competing_owner(self):
        f=Fake()
        def drift(api):
            if api.reads==2: api.state=dataclasses.replace(api.state,assignees=("other",))
        f.on_snapshot=drift
        self.assertEqual(bot.run(f.e,f),"refused");self.assertEqual(f.writes,[])
        self.assertEqual(f.state,initial(assignees=("other",)))

    def test_assignment_race_removes_only_added_actor(self):
        f=Fake()
        def drift(api,remove):
            if not remove: api.state=dataclasses.replace(api.state,assignees=("other","outsider"))
        f.on_assign=drift
        self.assertEqual(bot.run(f.e,f),"refused")
        self.assertEqual(f.writes,[("add","outsider"),("remove","outsider")])
        self.assertEqual(f.state,initial(assignees=("other",)))

    def test_manual_status_drift_stops_further_writes(self):
        f=Fake()
        def drift(api,remove):api.state=dataclasses.replace(api.state,status="Done")
        f.on_assign=drift
        self.assertEqual(bot.run(f.e,f),"refused")
        self.assertEqual(f.writes,[("add","outsider")]);self.assertEqual(f.state.status,"Done")

    def test_partial_api_failure_and_new_command_reconciliation(self):
        f=Fake()
        def fail(api,field):raise bot.ApiFailure("synthetic API failure")
        f.on_field=fail
        self.assertEqual(bot.run(f.e,f),"error")
        self.assertEqual(f.state,initial(assignees=("outsider",)))
        self.assertEqual(f.messages[-1][0],"error")
        f.on_field=None;f.e=event(comment=101)
        self.assertEqual(bot.run(f.e,f),"success");self.assertEqual(f.state.status,"In progress")

    def test_partial_release_can_be_reconciled_by_previous_owner(self):
        f=Fake();bot.run(f.e,f)
        f.e=event("/unclaim",comment=101)
        def fail(api,field):raise bot.ApiFailure("synthetic API failure")
        f.on_field=fail;self.assertEqual(bot.run(f.e,f),"error")
        self.assertEqual(f.state.assignees,())
        f.on_field=None;f.e=event("/unclaim",comment=102)
        self.assertEqual(bot.run(f.e,f),"success");self.assertEqual(f.state.status,"Ready")

    def test_partial_release_after_status_write_reconciles_own_agent_record(self):
        f=Fake();bot.run(f.e,f);f.e=event("/unclaim",comment=101)
        def fail_agent(api,field):
            if field=="a":raise bot.ApiFailure("synthetic Agent write failure")
        f.on_field=fail_agent
        self.assertEqual(bot.run(f.e,f),"error")
        self.assertEqual((f.state.status,f.state.assignees),("Ready",()))
        self.assertEqual(bot.owned_record(f.state.agent)["owner"],"outsider")
        f.on_field=None;f.e=event("/unclaim",comment=102)
        self.assertEqual(bot.run(f.e,f),"success")
        self.assertEqual(bot.owned_record(f.state.agent),{"actor":"outsider","owner":None,"request":102,"status":"Ready","pr":None})

    def test_free_unclaim_does_not_overwrite_tracking(self):
        f=Fake(event("/unclaim"));self.assertEqual(bot.run(f.e,f),"success");self.assertEqual(f.writes,[])

    def test_free_task_does_not_let_a_foreign_actor_edit_stale_owner_record(self):
        req=bot.authorize(event(),Fake());agent=bot.with_record(initial().agent,req,"outsider","In progress",None)
        f=Fake(event("/unclaim",actor="another",comment=101),initial(agent=agent))
        self.assertEqual(bot.run(f.e,f),"success")
        self.assertEqual(f.writes,[])
        self.assertEqual(f.state,initial(agent=agent))

    def test_agent_other_lines_retained_and_duplicate_record_refused(self):
        req=bot.authorize(event(),Fake())
        text=bot.with_record("Other\r\nOther2\n",req,"outsider","In progress",None)
        self.assertTrue(text.startswith("Other\r\nOther2\n"))
        with self.assertRaises(bot.Refusal):bot.with_record(text+"\n"+text,req,None,"Ready",None)


class Opener:
    def __init__(self,*answers):self.answers=list(answers);self.requests=[]
    def open(self,request,timeout):
        self.requests.append((request,timeout));answer=self.answers.pop(0)
        if isinstance(answer,Exception):raise answer
        return io.BytesIO(answer if isinstance(answer,bytes) else json.dumps(answer).encode())


def profile():
    return {"id":"p","closed":False,"viewerCanUpdate":True,"fields":{"pageInfo":{"hasNextPage":False},"nodes":[
        {"id":"s","name":"Status","__typename":"ProjectV2SingleSelectField","options":[{"name":n,"id":i} for n,i in initial().options]},
        {"id":"a","name":"Agent","__typename":"ProjectV2Field","dataType":"TEXT"}]}}


def snapshot_response():
    return {"data":{"user":{"projectV2":profile()},"repository":{"issue":{"number":10,"url":f"https://github.com/{bot.REPOSITORY}/issues/10","state":"OPEN","locked":False,
        "assignees":{"pageInfo":{"hasNextPage":False},"nodes":[]},"projectItems":{"pageInfo":{"hasNextPage":False},"nodes":[
        {"id":"i","isArchived":False,"project":{"id":"p"},"status":{"__typename":"ProjectV2ItemFieldSingleSelectValue","name":"Ready","optionId":"r"},"agent":{"__typename":"ProjectV2ItemFieldTextValue","text":initial().agent}}]}}}}}


class HttpContract(unittest.TestCase):
    def test_real_snapshot_parser(self):
        api=bot.GitHub("repo-test","project-test",Opener(snapshot_response()))
        self.assertEqual(api.snapshot(10),initial())

    def test_missing_secret_before_assignment(self):
        api=bot.GitHub("repo-test","",Opener())
        with self.assertRaises(bot.ApiFailure):api.snapshot(10)
        self.assertEqual(api.opener.requests,[])

    def test_project_schema_and_incomplete_lists_refused(self):
        for change in [lambda p:p.update(closed=True),lambda p:p.update(viewerCanUpdate=False),
            lambda p:p["fields"]["pageInfo"].update(hasNextPage=True),lambda p:p["fields"]["nodes"].pop(),
            lambda p:p["fields"]["nodes"][1].update(dataType="NUMBER"),lambda p:p["fields"]["nodes"][0].update(options=[o for o in p["fields"]["nodes"][0]["options"] if o["name"] != "Ready"])]:
            response=snapshot_response();change(response["data"]["user"]["projectV2"])
            with self.subTest(change=change),self.assertRaises(bot.Refusal):bot.GitHub("a","b",Opener(response)).snapshot(10)
        for section in ["assignees","projectItems"]:
            response=snapshot_response();response["data"]["repository"]["issue"][section]["pageInfo"]["hasNextPage"]=True
            with self.assertRaises(bot.Refusal):bot.GitHub("a","b",Opener(response)).snapshot(10)

    def test_missing_archived_or_mismatching_item_refused(self):
        for mutate in [lambda items:items.clear(),lambda items:items[0].update(isArchived=True),lambda items:items[0]["project"].update(id="other"),lambda items:items[0]["status"].update(optionId="wrong")]:
            response=snapshot_response();mutate(response["data"]["repository"]["issue"]["projectItems"]["nodes"])
            with self.assertRaises(bot.Refusal):bot.GitHub("a","b",Opener(response)).snapshot(10)

    def test_unknown_issue_profile_is_refused_before_tracking(self):
        for key,value in [("locked",None),("locked","false"),("state","UNKNOWN")]:
            response=snapshot_response();response["data"]["repository"]["issue"][key]=value
            with self.subTest(key=key,value=value),self.assertRaises(bot.Refusal):
                bot.GitHub("a","b",Opener(response)).snapshot(10)

    def test_tokens_are_separate_and_routes_fixed(self):
        opener=Opener({}, {"data":{"ok":True}});api=bot.GitHub("repo-test","project-test",opener)
        api.assign(10,"outsider");api.gql("query{viewer{login}}",{})
        self.assertEqual([r.headers["Authorization"] for r,_ in opener.requests],["Bearer repo-test","Bearer project-test"])
        self.assertEqual([r.full_url for r,_ in opener.requests],[f"https://api.github.com/repos/{bot.REPOSITORY}/issues/10/assignees","https://api.github.com/graphql"])
        self.assertEqual([t for _,t in opener.requests],[20,20])
        for path in ["https://evil.test", "/repos/other/repo/issues/10", "//evil.test"]:
            with self.assertRaises(bot.ApiFailure):api.http("GET",path)

    def test_errors_bounded_and_no_credentials_in_diagnostics(self):
        for answer in [b"not json",b"x"*(bot.MAX_RESPONSE+1),urllib.error.URLError("secret"),
            urllib.error.HTTPError("https://evil.test/secret",403,"secret",{},None),{"errors":[{"message":"secret"}]},TimeoutError("secret")]:
            with self.subTest(kind=type(answer)):
                with self.assertRaises(bot.ApiFailure) as caught:bot.GitHub("secret","secret",Opener(answer)).gql("query{}",{})
                self.assertNotIn("secret",str(caught.exception))

    def test_redirect_rejected(self):
        with self.assertRaises(bot.ApiFailure):bot.NoRedirect().redirect_request(None,None,302,"",{},"https://evil.test")

    def test_readonly_configuration(self):
        opener=Opener({"data":{"user":{"projectV2":profile()}}});api=bot.GitHub("a","b",opener);api.check_configuration()
        data=json.loads(opener.requests[0][0].data)
        self.assertNotIn("mutation",data["query"]);self.assertNotIn("repository(",data["query"])
        self.assertNotIn("$issue",data["query"])

    def test_receipt_ignores_human_forgery(self):
        req=bot.authorize(event(),Fake())
        row={"id":1,"user":{"login":"outsider","type":"User"},"body":bot.RECEIPT_PREFIX+json.dumps({"request":100,"digest":req.digest,"outcome":"success"})+" -->"}
        api=bot.GitHub("a","b",Opener([row]));self.assertEqual(api.receipts(req),[])
        row["user"]={"login":"github-actions[bot]","type":"Bot"}
        api=bot.GitHub("a","b",Opener([row]));self.assertEqual(api.receipts(req),[(1,{"request":100,"digest":req.digest,"outcome":"success"})])

    def test_receipt_pagination_is_bounded_and_malformed_metadata_refused(self):
        req=bot.authorize(event(),Fake())
        rows=[{"user":{"login":"outsider","type":"User"},"body":"hello"}]*100
        opener=Opener(*([rows]*20));api=bot.GitHub("a","b",opener)
        with self.assertRaises(bot.Refusal):api.receipts(req)
        self.assertEqual(len(opener.requests),20)
        self.assertIn("page=20",opener.requests[-1][0].full_url)
        row={"id":1,"user":{"login":"github-actions[bot]","type":"Bot"},"body":bot.RECEIPT_PREFIX+'{"request":100,"digest":"invalid","outcome":"success"} -->'}
        with self.assertRaises(bot.Refusal):bot.GitHub("a","b",Opener([row])).receipts(req)

    def test_receipt_write_requires_exact_readback(self):
        req=bot.authorize(event(),Fake())
        api=bot.GitHub("a","b",Opener({"id":1},{"id":1,"body":"different","user":{"login":"github-actions[bot]","type":"Bot"}}))
        with self.assertRaises(bot.ApiFailure):api.receipt(req,"pending","hello")


if __name__ == "__main__":unittest.main(verbosity=2)