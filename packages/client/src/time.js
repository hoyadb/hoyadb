// API-0011 exact host timer boundary; no queue/backoff planner lives here.
import {integer} from './values.js';
import {check} from './errors.js';
export function timerDelay(due,now){
 due=integer(due);now=integer(now);check(due[0]!=='-'&&now[0]!=='-');
 if(due.length<now.length||(due.length===now.length&&due<=now))return 1;
 const right=now.padStart(due.length,'0');let borrow=0,result='';
 for(let i=due.length-1;i>=0;i--){let digit=due.charCodeAt(i)-right.charCodeAt(i)-borrow;borrow=digit<0?1:0;if(borrow)digit+=10;result=String(digit)+result;}
 result=result.replace(/^0+/,'')||'0';const cap='2147483647';
 return result.length>cap.length||(result.length===cap.length&&result>cap)?2147483647:Math.max(1,Number(result));
}
