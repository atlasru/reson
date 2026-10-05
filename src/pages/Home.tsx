import {useEffect,useState} from 'react';
import {ArrowUpRight,Play} from 'lucide-react';
import {call,play,useLibrary,useProviders} from '../stores/core';
import type {Route,Track} from '../stores/types';
import {Artwork} from '../components/Artwork';
import {TrackList} from '../components/TrackList';
import {Failure,Skeleton} from '../components/States';
export function Home({navigate}:{navigate:(r:Route)=>void}) {
 const lib=useLibrary();const providers=useProviders();const [tracks,setTracks]=useState<Track[]>([]);const [loading,setLoading]=useState(true);const [error,setError]=useState('');const [retry,setRetry]=useState(0);const source=lib.recent[0]?.sources[0];const id=source?.provider_id;const provider=source?.provider??providers[0]?.id??'soundcloud';
 useEffect(()=>{let alive=true;setLoading(true);setError('');void call<Track[]>(id?'related_tracks':'discover',{provider,id:id??null}).then(r=>{if(alive)setTracks(r);}).catch(e=>{if(alive)setError(String(e));}).finally(()=>{if(alive)setLoading(false);});return()=>{alive=false;};},[id,provider,retry]);
 return <div className="page"><header className="page-title"><div><span className="eyebrow">Your music, in one place</span><h1>Home</h1></div><button className="secondary" onClick={()=>navigate({page:'search'})}>Find music <ArrowUpRight size={16}/></button></header>
 {lib.recent.length>0&&<section className="recent-strip"><h2>Pick up where you left off</h2><div>{lib.recent.slice(0,4).map(t=><button className="recent-track" key={t.internal_id} onClick={()=>play([t.internal_id])}><Artwork url={t.artwork}/><span><strong>{t.title}</strong><small>{t.artists.map(a=>a.name).join(', ')}</small></span><Play size={15}/></button>)}</div></section>}
 <div className="section-title"><h2>{id?'More like your last listen':'Trending on SoundCloud'}</h2><span>SoundCloud · Public catalog</span></div>
 {loading?<Skeleton/>:error?<Failure message={error} retry={()=>setRetry(n=>n+1)}/>:<TrackList tracks={tracks} navigate={navigate}/>}
 </div>;
}
