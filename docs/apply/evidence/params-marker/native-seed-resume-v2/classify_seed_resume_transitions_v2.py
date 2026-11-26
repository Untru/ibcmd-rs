"""Offline exact native storage header/payload transition inventory, no normalization."""
import sys
sys.dont_write_bytecode=True
import collections,json,pathlib
from check_seed_stage import Image,ID,inflate
LAB=pathlib.Path('F:/ibcmd/lab/05/wave3/params-marker')
OUT=LAB/'native-seed-resume-V2-storage-transitions-v1.json'
if OUT.exists():raise ValueError('fresh classification required')
def index(image,table):
    result={}
    for r in image.data['storage'][table]['rows']:
        key=(r['name'],r['part'])
        if key in result:raise ValueError('duplicate physical key')
        result[key]={k:v for k,v in r.items() if k!='offset'}
    return result
def state(image):
    rows=image.data['storage']['Params']['rows']
    marker=image.blob('Params','DynamicallyUpdated',True)
    descriptor=image.blob('Config',ID)
    return {'tag':image.folder.name,'save_count':len(image.data['storage']['ConfigSave']['rows']),
            'config_pending':[r for r in image.data['storage']['Config']['rows'] if '_dynupdate_' in r['name'] or r['name'] in ('DynamicallyUpdated','deleted')],
            'params_marker':next((r for r in rows if r['name']=='DynamicallyUpdated'),None),
            'params_marker_utf8':None if marker is None else marker.decode('utf-8-sig'),
            'params_generation_rows':[r for r in rows if '_dynupdate_' in r['name']],
            'SI_rows':[r for r in rows if r['name'].lower().endswith('.si') or 'siversions' in r['name'].lower()],
            'ordinary_descriptor_bytes':len(descriptor),'ordinary_descriptor_decoded_bytes':len(inflate(descriptor))}
observations=[];changes=[]
previous='seed-resume-initial-v2'
for phase in ('force_b','force_c','fold_d'):
    for action,after in (('import',phase+'-staged-v2'),('apply',phase+'-applied-v2')):
        a,b=Image(previous),Image(after);tables={}
        for table in a.data['storage']:
            left,right=index(a,table),index(b,table)
            added=[{'key':k,'after':right[k]} for k in sorted(right.keys()-left.keys())]
            removed=[{'key':k,'before':left[k]} for k in sorted(left.keys()-right.keys())]
            changed=[{'key':k,'fields':[x for x in left[k] if left[k][x]!=right[k][x]],'before':left[k],'after':right[k]} for k in sorted(left.keys()&right.keys()) if left[k]!=right[k]]
            tables[table]={'added':added,'removed':removed,'changed':changed,'unchanged_rows':sum(left[k]==right[k] for k in left.keys()&right.keys())}
        aux={}
        for key,value in a.data['auxiliary'].items():
            other=b.data['auxiliary'][key]
            if value['columns']!=other['columns']:raise ValueError('auxiliary schema changed')
            count=lambda v:collections.Counter(json.dumps(r,sort_keys=True,ensure_ascii=False,separators=(',',':')) for r in v['rows'])
            ca,cb=count(value),count(other)
            aux[key]={'before_rows':len(value['rows']),'after_rows':len(other['rows']),'removed_multiset_count':sum((ca-cb).values()),'added_multiset_count':sum((cb-ca).values()),'exact_equal':ca==cb}
        changes.append({'operation':phase+'-'+action,'before':previous,'after':after,'storage':tables,'auxiliary':aux})
        observations.append(state(b));previous=after
result={'scope':'Actual native-only six-operation physical inventory; N1/OWN acceptance not inferred','row_identity':'name+part; all captured database header fields and raw SHA compared; pack offsets excluded only from per-key presentation, full offset/pack proof retained in terminal analysis','normalizations':[],
        'initial':state(Image('seed-resume-initial-v2')),'observations':observations,'transitions':changes,
        'opaque_policy':'All UI/service/cache differences are retained exact headers/raw SHA/full snapshots. No licensing codec interpretation or whole Params parity claim.'}
with OUT.open('x',encoding='utf-8') as f:json.dump(result,f,ensure_ascii=False,indent=2);f.write('\n')
print(json.dumps({'transitions':len(changes),'final':observations[-1]['params_marker_utf8'],'ordinary_descriptor_bytes':observations[-1]['ordinary_descriptor_bytes'],'final_generation_rows':len(observations[-1]['params_generation_rows']),'N1_OWN_executed':False},ensure_ascii=False))
