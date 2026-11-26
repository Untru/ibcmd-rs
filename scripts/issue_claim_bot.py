#!/usr/bin/env python3
"""Bounded public issue commands; no third-party packages or contributor code."""
from __future__ import annotations

import dataclasses
import datetime
import hashlib
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

REPOSITORY = "Untru/ibcmd-rs"
PROJECT_OWNER = "Untru"
PROJECT_NUMBER = 9
AGENT_PREFIX = "Claim bot: "
RECEIPT_PREFIX = "<!-- ibcmd-issue-claim-bot:v1 "
LOGIN = re.compile(r"[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?\Z")
RECEIPT = re.compile(re.escape(RECEIPT_PREFIX) + r"(\{[^\n]*\}) -->\Z")
MAX_RESPONSE = 8 * 1024 * 1024


class Refusal(Exception):
    """A safe message suitable for a public contributor receipt."""


class ApiFailure(Exception):
    """Transport/API errors contain no response bodies or credentials."""


@dataclasses.dataclass(frozen=True)
class Request:
    issue: int
    comment: int
    actor: str
    actor_id: int
    body: str
    created_at: str

    @property
    def digest(self):
        data = [REPOSITORY, self.issue, self.comment, self.actor.casefold(), self.actor_id, self.body]
        return hashlib.sha256(json.dumps(data, ensure_ascii=True).encode()).hexdigest()


@dataclasses.dataclass(frozen=True)
class Snapshot:
    issue: int
    project: str
    item: str
    status_field: str
    agent_field: str
    options: tuple
    open: bool
    locked: bool
    assignees: tuple
    status: str
    agent: str


def positive(value):
    if type(value) is not int or value <= 0:
        raise Refusal("Неверная идентичность запроса.")
    return value


def command(body):
    value = body.strip()
    if value in ("/claim", "/unclaim"):
        return value[1:], None
    match = re.fullmatch(r"/review[ \t]+(?:#([1-9][0-9]*)|https://github\.com/(?i:Untru/ibcmd-rs)/pull/([1-9][0-9]*))", value)
    if match and len(value) <= 200:
        return "review", int(match[1] or match[2])
    raise Refusal("Команды: `/claim`, `/unclaim`, `/review #номер_PR`. Одна команда на комментарий.")


def owned_record(agent):
    own = [line[len(AGENT_PREFIX):] for line in agent.splitlines() if line.startswith(AGENT_PREFIX)]
    if len(own) > 1:
        raise Refusal("В Agent несколько записей бота; требуется сверка сопровождающего.")
    if not own:
        return None
    try:
        value = json.loads(own[0])
        if set(value) != {"actor", "owner", "request", "status", "pr"}:
            raise ValueError()
        if not isinstance(value["actor"], str) or not LOGIN.fullmatch(value["actor"]):
            raise ValueError()
        if value["owner"] is not None and (not isinstance(value["owner"], str) or not LOGIN.fullmatch(value["owner"])):
            raise ValueError()
        positive(value["request"])
        if value["pr"] is not None:
            positive(value["pr"])
        if value["status"] not in ("Ready", "In progress", "In review"):
            raise ValueError()
        return value
    except (ValueError, TypeError, KeyError, Refusal):
        raise Refusal("Запись бота в Agent повреждена; чужой текст не изменён.") from None


def with_record(agent, req, owner, status, pr):
    owned_record(agent)
    other = "".join(line for line in agent.splitlines(keepends=True) if not line.startswith(AGENT_PREFIX))
    if other and not other.endswith(("\n", "\r")):
        other += "\n"
    record = {"actor": req.actor, "owner": owner, "request": req.comment, "status": status, "pr": pr}
    value = other + AGENT_PREFIX + json.dumps(record, ensure_ascii=True, separators=(",", ":"))
    if len(value.encode("utf-16-le")) // 2 > 1024:
        raise Refusal("Поле Agent заполнено. Сопровождающему нужно освободить место; чужие записи не сокращались.")
    return value


def validate_pr(pr, req, number):
    reference = re.search(r"(?<![\w/#])#" + str(req.issue) + r"(?![0-9])", pr.get("body") or "")
    reference = reference or re.search(r"https://github\.com/(?i:Untru/ibcmd-rs)/issues/" + str(req.issue) + r"(?![0-9])", pr.get("body") or "")
    if (pr.get("number") != number or pr.get("html_url") != f"https://github.com/{REPOSITORY}/pull/{number}"
            or pr.get("state") != "open" or pr.get("draft") is not False
            or pr.get("user", {}).get("login", "").casefold() != req.actor.casefold()
            or pr.get("base", {}).get("repo", {}).get("full_name", "").casefold() != REPOSITORY.casefold()
            or pr.get("base", {}).get("ref") != "master" or not reference):
        raise Refusal("Нужен ваш открытый non-Draft PR в этом репозитории, в master, с явной ссылкой на эту issue.")


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ApiFailure("GitHub API перенаправил запрос; операция остановлена.")


class GitHub:
    PROJECT_QUERY = """query($issue:Int!){
      user(login:"Untru"){projectV2(number:9){id closed viewerCanUpdate fields(first:100){pageInfo{hasNextPage} nodes{
        __typename ...on ProjectV2Field{id name dataType}
        ...on ProjectV2SingleSelectField{id name options{id name}}
      }}}}
      repository(owner:"Untru",name:"ibcmd-rs"){issue(number:$issue){number url state locked
        assignees(first:20){pageInfo{hasNextPage} nodes{login}}
        projectItems(first:100){pageInfo{hasNextPage} nodes{id isArchived project{id}
          status:fieldValueByName(name:"Status"){__typename ...on ProjectV2ItemFieldSingleSelectValue{name optionId}}
          agent:fieldValueByName(name:"Agent"){__typename ...on ProjectV2ItemFieldTextValue{text}}
        }}
      }}
    }"""

    def __init__(self, token, project_token, opener=None, *, single_writer_confirmed=False):
        if not token:
            raise ApiFailure("Нет GITHUB_TOKEN для обработки issue.")
        self.single_writer_confirmed = single_writer_confirmed is True
        self.token = token
        self.project_token = project_token
        self.opener = opener or urllib.request.build_opener(NoRedirect())

    def http(self, method, path, data=None, project=False):
        token = self.project_token if project else self.token
        if not token:
            raise ApiFailure("Не настроен Actions Secret PROJECT_TOKEN; назначение и доска не изменены.")
        if path != "/graphql" and not path.startswith(f"/repos/{REPOSITORY}/"):
            raise ApiFailure("Недопустимый маршрут GitHub API.")
        request = urllib.request.Request("https://api.github.com" + path,
            data=None if data is None else json.dumps(data).encode(), method=method,
            headers={"Authorization": "Bearer " + token, "Accept": "application/vnd.github+json",
                     "X-GitHub-Api-Version": "2022-11-28", "User-Agent": "ibcmd-issue-claim-bot",
                     "Content-Type": "application/json"})
        try:
            with self.opener.open(request, timeout=20) as response:
                raw = response.read(MAX_RESPONSE + 1)
                if len(raw) > MAX_RESPONSE:
                    raise ApiFailure("Ответ GitHub API превышает лимит; операция остановлена.")
                return json.loads(raw) if raw else None
        except urllib.error.HTTPError as error:
            raise ApiFailure(f"GitHub API вернул HTTP {error.code}; состояние операции не подтверждено.") from None
        except (urllib.error.URLError, TimeoutError, OSError, ValueError):
            raise ApiFailure("Нет проверенного ответа GitHub API; состояние операции не подтверждено.") from None

    def gql(self, query, variables):
        answer = self.http("POST", "/graphql", {"query": query, "variables": variables}, project=True)
        if not isinstance(answer, dict) or answer.get("errors") or not answer.get("data"):
            raise ApiFailure("Project API не подтвердил запрос; проверьте PROJECT_TOKEN и права проекта.")
        return answer["data"]

    @staticmethod
    def profile(project):
        if not project or project.get("closed") is not False or project.get("viewerCanUpdate") is not True:
            raise Refusal("Открытый Project №9 и права его обновления не подтверждены.")
        fields = project["fields"]
        if fields["pageInfo"]["hasNextPage"]:
            raise Refusal("Неполная схема полей Project; изменений нет.")
        named = {}
        for field in fields["nodes"]:
            if field and field.get("name") in ("Status", "Agent"):
                if field["name"] in named:
                    raise Refusal("Неоднозначные поля Project; изменений нет.")
                named[field["name"]] = field
        status, agent = named.get("Status"), named.get("Agent")
        if (not status or status["__typename"] != "ProjectV2SingleSelectField"
                or not agent or agent["__typename"] != "ProjectV2Field" or agent["dataType"] != "TEXT"):
            raise Refusal("Нужны поля Status (single select) и Agent (text).")
        options = {x["name"]: x["id"] for x in status["options"]}
        if len(options) != len(status["options"]) or not {"Ready", "In progress", "In review"} <= options.keys():
            raise Refusal("Схема статусов Project не подтверждена.")
        return status, agent, options

    def check_configuration(self):
        query = self.PROJECT_QUERY.split('      repository(', 1)[0] + '}'
        query = query.replace('query($issue:Int!)', 'query')
        data = self.gql(query, {})
        self.profile((data.get("user") or {}).get("projectV2"))

    def snapshot(self, number):
        data = self.gql(self.PROJECT_QUERY, {"issue": number})
        project = (data.get("user") or {}).get("projectV2")
        status, agent, options = self.profile(project)
        issue = (data.get("repository") or {}).get("issue")
        if not issue or issue.get("number") != number or issue.get("url") != f"https://github.com/{REPOSITORY}/issues/{number}":
            raise Refusal("Issue не принадлежит настроенному репозиторию.")
        if issue.get("state") not in ("OPEN", "CLOSED") or type(issue.get("locked")) is not bool:
            raise Refusal("Состояние issue не подтверждено; изменений нет.")
        items = issue["projectItems"]
        if items["pageInfo"]["hasNextPage"] or issue["assignees"]["pageInfo"]["hasNextPage"]:
            raise Refusal("Неполный список проекта или исполнителей; изменений нет.")
        matches = [x for x in items["nodes"] if x["project"]["id"] == project["id"]]
        if len(matches) != 1 or matches[0]["isArchived"]:
            raise Refusal("Задача должна быть добавлена в Project №9 со статусом Backlog или Ready.")
        item = matches[0]
        sv, av = item["status"], item["agent"]
        if (not sv or sv.get("__typename") != "ProjectV2ItemFieldSingleSelectValue"
                or sv.get("optionId") != options.get(sv.get("name"))
                or (av is not None and av.get("__typename") != "ProjectV2ItemFieldTextValue")):
            raise Refusal("Значения полей Project не подтверждены.")
        text = "" if av is None else av["text"]
        if not isinstance(text, str):
            raise Refusal("Неизвестное значение Agent.")
        return Snapshot(number, project["id"], item["id"], status["id"], agent["id"],
            tuple(sorted(options.items())), issue["state"] == "OPEN", bool(issue["locked"]),
            tuple(sorted(x["login"].casefold() for x in issue["assignees"]["nodes"])), sv["name"], text)

    def comment(self, number):
        return self.http("GET", f"/repos/{REPOSITORY}/issues/comments/{number}")

    def receipts(self, req):
        found = []
        for page in range(1, 21):
            query = urllib.parse.urlencode({"since": req.created_at, "per_page": 100, "page": page})
            rows = self.http("GET", f"/repos/{REPOSITORY}/issues/{req.issue}/comments?{query}")
            if not isinstance(rows, list):
                raise ApiFailure("Список квитанций GitHub не подтверждён.")
            for row in rows:
                if row.get("user", {}).get("login") != "github-actions[bot]" or row["user"].get("type") != "Bot":
                    continue
                match = RECEIPT.search(row.get("body") or "")
                if match:
                    try:
                        value = json.loads(match[1])
                        positive(value["request"])
                        if set(value) != {"request", "digest", "outcome"} or not re.fullmatch("[0-9a-f]{64}", value["digest"]):
                            raise ValueError()
                        if value["outcome"] not in ("pending", "success", "refused", "error"):
                            raise ValueError()
                    except (ValueError, TypeError, KeyError, Refusal):
                        raise Refusal("Повреждена квитанция бота; требуется сверка сопровождающего.") from None
                    found.append((row["id"], value))
            if len(rows) < 100:
                return found
        raise Refusal("Список квитанций слишком велик; обработка остановлена до изменений.")

    def receipt(self, req, outcome, message, receipt_id=None):
        value = {"request": req.comment, "digest": req.digest, "outcome": outcome}
        body = message + "\n\n" + RECEIPT_PREFIX + json.dumps(value, separators=(",", ":")) + " -->"
        path = (f"/repos/{REPOSITORY}/issues/{req.issue}/comments" if receipt_id is None
                else f"/repos/{REPOSITORY}/issues/comments/{receipt_id}")
        answer = self.http("POST" if receipt_id is None else "PATCH", path, {"body": body})
        actual = self.comment(answer["id"])
        if (actual.get("body") != body or actual.get("user", {}).get("login") != "github-actions[bot]"
                or actual["user"].get("type") != "Bot"
                or actual.get("issue_url") != f"https://api.github.com/repos/{REPOSITORY}/issues/{req.issue}"):
            raise ApiFailure("Квитанция бота не подтверждена.")
        return actual["id"]

    def assign(self, issue, actor, remove=False):
        self.http("DELETE" if remove else "POST", f"/repos/{REPOSITORY}/issues/{issue}/assignees", {"assignees": [actor]})

    def field(self, state, field, value):
        query = """mutation($project:ID!,$item:ID!,$field:ID!,$value:ProjectV2FieldValue!){
          updateProjectV2ItemFieldValue(input:{projectId:$project,itemId:$item,fieldId:$field,value:$value}){
            projectV2Item{id}}}"""
        self.gql(query, {"project": state.project, "item": state.item, "field": field, "value": value})

    def pull(self, number):
        return self.http("GET", f"/repos/{REPOSITORY}/pulls/{number}")


def authorize(event, api):
    if event.get("action") != "created" or event.get("repository", {}).get("full_name", "").casefold() != REPOSITORY.casefold():
        return None
    issue, comment = event.get("issue", {}), event.get("comment", {})
    if "pull_request" in issue or comment.get("user", {}).get("type") != "User":
        return None
    body = comment.get("body")
    if not isinstance(body, str) or not body.strip().startswith(("/claim", "/unclaim", "/review")):
        return None
    number, comment_id = positive(issue.get("number")), positive(comment.get("id"))
    user = comment["user"]
    actor = user.get("login")
    if not isinstance(actor, str) or not LOGIN.fullmatch(actor) or event.get("sender", {}).get("id") != user.get("id"):
        raise Refusal("Автор комментария не подтверждён.")
    actual = api.comment(comment_id)
    if (actual.get("id") != comment_id or actual.get("issue_url") != f"https://api.github.com/repos/{REPOSITORY}/issues/{number}"
            or actual.get("body") != body or actual.get("user", {}).get("id") != positive(user.get("id"))
            or actual["user"].get("login", "").casefold() != actor.casefold() or actual["user"].get("type") != "User"
            or actual.get("created_at") != comment.get("created_at")):
        raise Refusal("Комментарий изменён или его привязка не подтверждена; отправьте новую команду.")
    try:
        datetime.datetime.strptime(actual["created_at"], "%Y-%m-%dT%H:%M:%SZ")
    except (ValueError, TypeError):
        raise Refusal("Время комментария не подтверждено.") from None
    return Request(number, comment_id, actor, user["id"], body, actual["created_at"])


def perform(api, req):
    action, pr = command(req.body)
    state = api.snapshot(req.issue)
    if not state.open or state.locked:
        raise Refusal("Закрытая или заблокированная issue не принимает команды.")
    record = owned_record(state.agent)
    if record and req.comment <= record["request"]:
        raise Refusal("Команда устарела; отправьте новую команду после проверки текущего состояния.")
    actor = req.actor.casefold()
    if len(state.assignees) > 1:
        raise Refusal("Несколько исполнителей требуют сверки сопровождающего; назначения не заменялись.")
    if action == "claim":
        if state.assignees not in ((), (actor,)):
            raise Refusal("Задача уже назначена другому участнику.")
        if state.status == "In review":
            raise Refusal("Задача находится в In review. Команда /claim не возвращает задачу с ревью в работу.")
        if (not state.assignees and state.status not in ("Backlog", "Ready")) or (state.assignees and state.status not in ("Backlog", "Ready", "In progress")):
            raise Refusal("Команда /claim доступна для свободной задачи в Backlog или Ready; текущий статус не допускает назначение.")
        owner, status = req.actor, "In progress"
    elif action == "unclaim":
        if (not state.assignees and state.status == "Ready"
                and not (record and record["owner"] and record["owner"].casefold() == actor)):
            return "Задача уже свободна; назначения и доска не менялись."
        if state.assignees != (actor,) and not (not state.assignees and record and record["owner"] and record["owner"].casefold() == actor):
            raise Refusal("Освободить задачу может только её исполнитель.")
        if state.status not in ("Ready", "In progress", "In review"):
            raise Refusal("Текущий статус не допускает освобождение.")
        owner, status = None, "Ready"
    else:
        if state.assignees != (actor,) or state.status not in ("In progress", "In review"):
            raise Refusal("Запросить review может только текущий исполнитель активной задачи.")
        validate_pr(api.pull(pr), req, pr)
        owner, status = req.actor, "In review"
    text = with_record(state.agent, req, owner, status, pr)

    def change(kind, target):
        nonlocal state
        # This guard detects observable drift; it is not a conditional API write.
        # Direct editors must pause ingestion and drain the queue (see the runbook).
        if api.snapshot(req.issue) != state:
            raise Refusal("Состояние изменилось во время команды; дальнейшие записи остановлены. Проверьте назначение.")
        if kind == "add":
            api.assign(req.issue, req.actor)
        elif kind == "remove":
            api.assign(req.issue, req.actor, remove=True)
        elif kind == "status":
            api.field(state, state.status_field, {"singleSelectOptionId": dict(state.options)[target.status]})
        else:
            api.field(state, state.agent_field, {"text": target.agent})
        actual = api.snapshot(req.issue)
        if actual != target:
            # Undo only this newly added actor after a competing assignment, never another owner.
            if (kind == "add" and actor in actual.assignees and actual.assignees != (actor,)
                    and actual.status == state.status and actual.agent == state.agent and api.snapshot(req.issue) == actual):
                api.assign(req.issue, req.actor, remove=True)
                if api.snapshot(req.issue) != dataclasses.replace(actual, assignees=tuple(x for x in actual.assignees if x != actor)):
                    raise ApiFailure("Гонка назначений: снятие только нового назначения не подтверждено.")
            raise Refusal("Запись не совпала с ожидаемым состоянием; успех не подтверждён. Проверьте назначение и повторите новой командой.")
        state = actual

    if action == "claim" and not state.assignees:
        change("add", dataclasses.replace(state, assignees=(actor,)))
    if action == "unclaim" and state.assignees:
        change("remove", dataclasses.replace(state, assignees=()))
    if state.status != status:
        change("status", dataclasses.replace(state, status=status))
    if state.agent != text:
        change("agent", dataclasses.replace(state, agent=text))
    if action == "review":
        validate_pr(api.pull(pr), req, pr)
    if api.snapshot(req.issue) != state:
        raise Refusal("Итоговое состояние изменилось; успех не подтверждён.")
    if action == "claim":
        return f"@{req.actor}, задача закреплена за вами. На доске: **In progress**. Можно начинать работу."
    if action == "unclaim":
        return f"@{req.actor}, ваше назначение снято. На доске: **Ready**."
    return f"@{req.actor}, PR #{pr} связан с задачей. На доске: **In review**. Критерии приёмки и Done проверяет сопровождающий."


def run(event, api):
    req = authorize(event, api)
    if req is None:
        return "ignored"
    receipts = api.receipts(req)
    existing = [x for x in receipts if x[1]["request"] == req.comment]
    if existing:
        if len(existing) != 1 or existing[0][1]["digest"] != req.digest:
            raise Refusal("Квитанция повторного запроса не совпадает.")
        return "duplicate"
    receipt_id = api.receipt(req, "pending", f"@{req.actor}, проверяю команду. Начинайте работу только после итогового подтверждения; при незавершённом запуске отправьте новую команду.")
    try:
        if getattr(api, "single_writer_confirmed", False) is not True:
            raise ApiFailure("Владелец ещё не подтвердил режим единственного редактора: ISSUE_BOT_SINGLE_WRITER_CONFIRMED должен быть true. См. docs/issue-self-service.md.")
        if any(value["request"] > req.comment for _, value in receipts):
            raise Refusal("Более новая команда уже зарегистрирована; старая команда не применяется.")
        result = perform(api, req)
    except Refusal as error:
        api.receipt(req, "refused", "Команда не принята: " + str(error), receipt_id)
        return "refused"
    except ApiFailure as error:
        api.receipt(req, "error", "Успех не подтверждён: " + str(error) + " Проверьте текущее назначение и отправьте новую команду; при частичной записи может понадобиться /claim или /unclaim своего назначения.", receipt_id)
        return "error"
    api.receipt(req, "success", result, receipt_id)
    return "success"


def main():
    try:
        api = GitHub(os.environ.get("GITHUB_TOKEN", ""), os.environ.get("PROJECT_TOKEN", ""),
            single_writer_confirmed=os.environ.get("ISSUE_BOT_SINGLE_WRITER_CONFIRMED") == "true")
        if sys.argv[1:] == ["--check-config"]:
            api.check_configuration()
            print("Project №9: схема и право обновления подтверждены; записи не выполнялись.")
            return 0
        with open(os.environ["GITHUB_EVENT_PATH"], encoding="utf-8") as stream:
            event = json.load(stream)
        result = run(event, api)
        print(result)
        return 1 if result == "error" else 0
    except (ApiFailure, Refusal) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
