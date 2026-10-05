import {useEffect,useState} from 'react';
import {Play,Plus,Heart,ExternalLink} from 'lucide-react';
import {call,enqueue,favorite,original,play,useLibrary} from '../stores/core';
import type {Route,Track} from '../stores/types';
import {Artwork} from '../components/Artwork';
import {TrackList,time} from '../components/TrackList';
import {Failure,Skeleton} from '../components/States';
export function TrackView({track,navigate}:{track:Track;navigate:(r:Route)=>void}) {
 const lib=useLibrary();const [related,setRelated]=useState<Track[]>([]);const [loading,setLoading]=useState(true);const [error,setError]=useState('');const [retry,setRetry]=useState(0);const ref=track.sources[0];const liked=lib.favorites.some(t=>t.internal_id===track.internal_id);
 useEffect(()=>{let alive=true;setLoading(true);setError('');void call<Track[]>('related_tracks',{provider:ref.provider,id:ref.provider_id}).then(r=>{if(alive)setRelated(r);}).catch(e=>{if(alive)setError(String(e));}).finally(()=>{if(alive)setLoading(false);});return()=>{alive=false;};},[ref.provider,ref.provider_id,retry]);
 return <div className="page"><header className="entity-header"><Artwork url={track.artwork}/><div><span className="eyebrow">Track · {ref.provider}</span><h1>{track.title}</h1><p className="description">{track.artists.map(a=>a.name).join(', ')} · {time(track.duration_ms)}{track.availability==='preview'?' · Preview':track.availability==='unavailable'?' · Unavailable':''}</p><div className="button-row"><button className="primary" disabled={track.availability==='unavailable'} onClick={()=>play([track.internal_id])}><Play size={16} fill="currentColor"/>Play</button><button className="secondary" onClick={()=>enqueue([track.internal_id])}><Plus size={16}/>Queue</button><button className={`icon ${liked?'active':''}`} title="Reson favorite" onClick={()=>favorite(track.internal_id,!liked)}><Heart size={19} fill={liked?'currentColor':'none'}/></button>{ref.url&&<button className="icon" title="Open on source" onClick={()=>original(ref.url!)}><ExternalLink size={17}/></button>}</div></div></header><div className="section-title"><h2>Related tracks</h2><span>SoundCloud</span></div>{loading?<Skeleton/>:error?<Failure message={error} retry={()=>setRetry(n=>n+1)}/>:<TrackList tracks={related} navigate={navigate}/>}</div>;
}
