import {check,failure} from './errors.js';
// This is byte-boundary validation, not a second JSON/domain parser.
export function utf8Length(text,max=Infinity){
 check(typeof text==='string');let bytes=0;
 for(let n=0;n<text.length;n++){
  const code=text.charCodeAt(n);
  if(code<0x80)bytes++;
  else if(code<0x800)bytes+=2;
  else if(code>=0xd800&&code<=0xdbff){const low=text.charCodeAt(++n);if(!(low>=0xdc00&&low<=0xdfff))throw failure('InvalidPayload');bytes+=4;}
  else if(code>=0xdc00&&code<=0xdfff)throw failure('InvalidPayload');
  else bytes+=3;
  if(bytes>max)throw failure('LimitExceeded');
 }
 return bytes;
}
