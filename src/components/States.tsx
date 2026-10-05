import {CloudOff,SearchX,RotateCcw} from 'lucide-react';
export function Empty({title,detail}:{title:string;detail?:string}){return <div className="empty"><SearchX size={28}/><h3>{title}</h3>{detail&&<p>{detail}</p>}</div>;}
export function Failure({message,retry}:{message:string;retry?:()=>void}){return <div className="empty error-state"><CloudOff size={30}/><h3>Couldn’t load this page</h3><p>{message}</p>{retry&&<button className="secondary" onClick={retry}><RotateCcw size={15}/>Try again</button>}</div>;}
export function Skeleton(){return <div className="skeleton-list" aria-label="Loading" role="status">{Array.from({length:8},(_,i)=><div className="skeleton-row" key={i}><span/><div><b/><i/></div></div>)}</div>;}
