import {useRef,useState,useEffect} from 'react';
import {createPortal} from 'react-dom';
import {useVirtualizer} from '@tanstack/react-virtual';
import {Heart,MoreHorizontal,Play,Plus,SkipForward,ExternalLink,UserRound,ListMusic,Trash2} from 'lucide-react';
import {Artwork} from './Artwork';
import {Empty} from './States';
import {act,enqueue,favorite,original,play,useLibrary} from '../stores/core';
import type {Route,Track} from '../stores/types';
export const time=(ms:number)=>`${Math.floor(ms/60000)}:${String(Math.floor(ms/1000)%60).padStart(2,'0')}`;
export function TrackList({tracks,navigate,onRemove}:{tracks:Track[];navigate:(r:Route)=>void;onRemove?:(index:number)=>void}) {
 const parent=useRef<HTMLDivElement>(null);const lib=useLibrary();const liked=new Set(lib.favorites.map(t=>t.internal_id));
 const [menu,setMenu]=useState<{track:Track;index:number;x:number;y:number}|null>(null);
 const virtual=useVirtualizer({count:tracks.length,getScrollElement:()=>parent.current,estimateSize:()=>62,overscan:8,getItemKey:i=>tracks[i].internal_id+':'+i});
 useEffect(()=>{if(!menu)return;const close=()=>setMenu(null);window.addEventListener('click',close);window.addEventListener('blur',close);const key=(e:KeyboardEvent)=>{if(e.key==='Escape')close();};window.addEventListener('keydown',key);return()=>{window.removeEventListener('click',close);window.removeEventListener('blur',close);window.removeEventListener('keydown',key);};},[menu]);
 const openMenu=(track:Track,index:number,e:React.MouseEvent)=>{e.preventDefault();e.stopPropagation();setMenu({track,index,x:Math.min(e.clientX,window.innerWidth-250),y:Math.min(e.clientY,window.innerHeight-350)});};
 if(!tracks.length)return <Empty title="No tracks here yet" detail="Search for music or add tracks to your library."/>;
 return <><div className="track-head"><span>#</span><span>Track</span><span>Artist</span><span>Time</span><span/></div><div className="track-scroll" ref={parent}><div style={{height:virtual.getTotalSize(),position:'relative'}}>{virtual.getVirtualItems().map(row=>{const t=tracks[row.index];return <div key={row.key} className={`track-row ${t.availability==='unavailable'?'unavailable':''}`} style={{position:'absolute',top:0,left:0,width:'100%',height:row.size,transform:`translateY(${row.start}px)`}} onDoubleClick={()=>play(tracks.map(t=>t.internal_id),row.index)} onContextMenu={e=>openMenu(t,row.index,e)}>
 <button className="row-number icon" title={`Play ${t.title}`} onClick={()=>play(tracks.map(t=>t.internal_id),row.index)}><span>{row.index+1}</span><Play size={15}/></button>
 <button className="track-name" onClick={()=>navigate({page:'track',track:t})}><Artwork url={t.artwork}/><span><strong>{t.title}</strong><small>{t.availability==='preview'?'Preview · ':t.availability==='unavailable'?'Unavailable · ':''}{t.explicit&&<b className="explicit">E</b>}{t.sources[0]?.provider==='soundcloud'?'SoundCloud':''}</small></span></button>
 <button className="artist-name" onClick={()=>{const ref=t.artists[0]?.references[0];if(ref)navigate({page:'artist',provider:ref.provider,id:ref.provider_id});}}>{t.artists.map(a=>a.name).join(', ')}</button><span className="duration">{time(t.duration_ms)}</span>
 <div className="row-actions"><button className={`icon ${liked.has(t.internal_id)?'active':''}`} title={liked.has(t.internal_id)?'Remove Reson favorite':'Favorite in Reson'} onClick={()=>favorite(t.internal_id,!liked.has(t.internal_id))}><Heart size={16} fill={liked.has(t.internal_id)?'currentColor':'none'}/></button><button className="icon" title="Track actions" onClick={e=>openMenu(t,row.index,e)}><MoreHorizontal size={18}/></button></div>
 </div>;})}</div></div>{menu&&createPortal(<div className="context-menu" role="menu" style={{left:menu.x,top:menu.y}} onClick={e=>e.stopPropagation()}>
 <button onClick={()=>{play([menu.track.internal_id]);setMenu(null);}}><Play size={15}/>Play now</button><button onClick={()=>{enqueue([menu.track.internal_id],true);setMenu(null);}}><SkipForward size={15}/>Play next</button><button onClick={()=>{enqueue([menu.track.internal_id]);setMenu(null);}}><Plus size={15}/>Add to queue</button><button onClick={()=>{favorite(menu.track.internal_id,!liked.has(menu.track.internal_id));setMenu(null);}}><Heart size={15}/>{liked.has(menu.track.internal_id)?'Remove favorite':'Favorite in Reson'}</button>
 <button onClick={()=>{const ref=menu.track.artists[0]?.references[0];if(ref)navigate({page:'artist',provider:ref.provider,id:ref.provider_id});setMenu(null);}}><UserRound size={15}/>Open artist</button>
 {lib.playlists.length>0&&<><div className="menu-label">Add to local playlist</div><div className="menu-playlists">{lib.playlists.map(p=><button key={p.internal_id} onClick={()=>{act('add_to_playlist',{id:p.internal_id,ids:[menu.track.internal_id]});setMenu(null);}}><ListMusic size={14}/>{p.title}</button>)}</div></>}
 {menu.track.sources[0]?.url&&<button onClick={()=>{original(menu.track.sources[0].url!);setMenu(null);}}><ExternalLink size={15}/>Open source</button>}{onRemove&&<button onClick={()=>{onRemove(menu.index);setMenu(null);}}><Trash2 size={15}/>Remove from playlist</button>}
 </div>,document.body)}</>;
}
