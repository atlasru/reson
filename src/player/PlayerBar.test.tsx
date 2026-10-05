import {fireEvent,render,screen,cleanup} from '@testing-library/react';
import {afterEach,describe,it,expect,vi} from 'vitest';
import {PlayerBar} from './PlayerBar';
const {control,state}=vi.hoisted(()=>({control:vi.fn(),state:{status:'playing',current:{internal_id:'track',title:'Current song',artists:[{name:'Artist'}],artwork:null},entry_id:'entry',position_ms:10000,duration_ms:120000,volume:.7,shuffle:false,repeat:'off',error:null}}));
vi.mock('../stores/core',()=>({control,usePlayer:()=>state,useLibrary:()=>({favorites:[]}),favorite:vi.fn(),call:vi.fn(()=>Promise.reject('test'))}));
vi.mock('../components/Artwork',()=>({Artwork:()=>null}));
afterEach(()=>{cleanup();vi.clearAllMocks();});
describe('core playback controls',()=>{
 it('commits seeking only when the gesture finishes',()=>{render(<PlayerBar queueOpen={false} onQueue={()=>{}} navigate={()=>{}}/>);const slider=screen.getByRole('slider',{name:'Playback position'});fireEvent.change(slider,{target:{value:'45000'}});expect(control).not.toHaveBeenCalled();fireEvent.pointerUp(slider);expect(control).toHaveBeenCalledWith('seek',45000);});
 it('sends transport and queue mode changes to the core',()=>{render(<PlayerBar queueOpen={false} onQueue={()=>{}} navigate={()=>{}}/>);fireEvent.click(screen.getByRole('button',{name:'Pause'}));expect(control).toHaveBeenCalledWith('toggle');fireEvent.click(screen.getByTitle('Repeat: off'));expect(control).toHaveBeenCalledWith('repeat','queue');fireEvent.click(screen.getByTitle('Shuffle'));expect(control).toHaveBeenCalledWith('shuffle',true);});
});
