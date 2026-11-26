// TC-0045 DOC04: actual local-file Chrome rendering, search and keyboard focus.
// Uses an installed Chrome and Node's built-in WebSocket; downloads no packages.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createHash} from 'node:crypto';
import {mkdtemp, readFile, writeFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const profile = await mkdtemp(join(tmpdir(), 'hoyadb-docs-browser-'));
const executable = process.env.HOYADB_CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const browser = spawn(executable, ['--headless=new', '--no-first-run', '--no-default-browser-check', '--disable-background-networking', '--remote-debugging-port=0', `--user-data-dir=${profile}`, 'about:blank'], {stdio: 'ignore'});
let socket;
try {
  let port;
  for (let n = 0; n < 100; n++) {
    try {port = (await readFile(join(profile, 'DevToolsActivePort'), 'utf8')).split('\n'); break;}
    catch {await new Promise(resolve => setTimeout(resolve, 100));}
  }
  assert.ok(port, 'installed Chrome must start its local debugging endpoint');
  socket = new WebSocket(`ws://127.0.0.1:${port[0]}${port[1]}`);
  await new Promise((resolve, reject) => {socket.onopen = resolve; socket.onerror = reject;});
  let sequence = 0;
  const pending = new Map();
  const requests = [];
  socket.onmessage = event => {
    const result = JSON.parse(event.data);
    if (result.method === 'Network.requestWillBeSent') requests.push(result.params.request.url);
    if (result.id && pending.has(result.id)) {pending.get(result.id)(result); pending.delete(result.id);}
  };
  async function call(method, params = {}, sessionId) {
    const id = ++sequence;
    const result = new Promise(resolve => pending.set(id, resolve));
    socket.send(JSON.stringify({id, method, params, ...(sessionId ? {sessionId} : {})}));
    const value = await Promise.race([result, new Promise((_, reject) => setTimeout(() => reject(Error(`timeout ${method}`)), 10000))]);
    assert.equal(value.error, undefined, JSON.stringify(value.error));
    return value.result;
  }
  const {targetId} = await call('Target.createTarget', {url:'about:blank'});
  const {sessionId} = await call('Target.attachToTarget', {targetId, flatten:true});
  const c = (method, params) => call(method, params, sessionId);
  await c('Page.enable');
  await c('Network.enable');
  async function evaluate(expression) {
    const value = await c('Runtime.evaluate', {expression, returnByValue:true, awaitPromise:true});
    assert.equal(value.exceptionDetails, undefined, JSON.stringify(value.exceptionDetails));
    return value.result.value;
  }
  const url = pathToFileURL(resolve('docs/public/dist/index.html')).href;
  await c('Page.navigate', {url});
  for (let n = 0; n < 100; n++) {
    if (await evaluate('document.readyState === "complete" && !!document.getElementById("search")')) break;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  assert.equal(await evaluate('location.protocol'), 'file:');
  await evaluate('document.getElementById("search").value="migrations"; document.getElementById("search").dispatchEvent(new Event("input"))');
  assert.ok(await evaluate('document.getElementById("search-results").textContent.includes("Changing a schema")'));
  await evaluate('document.getElementById("search").value="zzzz-no-page"; document.getElementById("search").dispatchEvent(new Event("input"))');
  assert.equal(await evaluate('document.getElementById("search-results").textContent'), 'No pages found. Try another term.');
  await evaluate('document.getElementById("search").value=""; document.getElementById("search").dispatchEvent(new Event("input")); document.activeElement.blur()');
  await c('Input.dispatchKeyEvent', {type:'keyDown', key:'Tab', code:'Tab', windowsVirtualKeyCode:9});
  await c('Input.dispatchKeyEvent', {type:'keyUp', key:'Tab', code:'Tab', windowsVirtualKeyCode:9});
  const focus = await evaluate('({text:document.activeElement.textContent,outline:getComputedStyle(document.activeElement).outlineStyle,top:document.activeElement.getBoundingClientRect().top})');
  assert.equal(focus.text, 'Skip to content');
  assert.equal(focus.outline, 'solid');
  assert.ok(focus.top >= 0);
  const sizes = [];
  for (const width of [1440, 768, 320]) {
    await c('Emulation.setDeviceMetricsOverride', {width, height:900, deviceScaleFactor:1, mobile:false});
    const size = await evaluate('({width:innerWidth,scroll:document.documentElement.scrollWidth,main:document.querySelector("main").getBoundingClientRect().width})');
    assert.equal(size.width, width);
    assert.ok(size.scroll <= width, JSON.stringify(size));
    sizes.push(size);
  }
  await evaluate('document.querySelector("main").scrollIntoView()');
  const screenshot = await c('Page.captureScreenshot', {format:'png'});
  await writeFile('target/public-documentation/mobile.png', Buffer.from(screenshot.data,'base64'));
  await c('Emulation.setDeviceMetricsOverride', {width:1440, height:1000, deviceScaleFactor:1, mobile:false});
  await evaluate('scrollTo(0,0)');
  const desktop = await c('Page.captureScreenshot', {format:'png'});
  await writeFile('target/public-documentation/desktop.png', Buffer.from(desktop.data,'base64'));
  assert.ok(requests.length > 0);
  assert.ok(requests.every(url => url.startsWith('file:') || url.startsWith('data:')), 'site must load only local/embedded resources');
  const pages = JSON.parse(await readFile('docs/public/site.json', 'utf8'));
  await c('Emulation.setDeviceMetricsOverride', {width:320, height:900, deviceScaleFactor:1, mobile:false});
  for (const page of pages) {
    const pageUrl = pathToFileURL(resolve(`docs/public/dist/${page.slug}.html`)).href;
    await c('Page.navigate', {url:pageUrl});
    let loaded = false;
    for (let n = 0; n < 100; n++) {
      loaded = await evaluate(`document.readyState === 'complete' && location.href === ${JSON.stringify(pageUrl)} && !!document.querySelector('main')`);
      if (loaded) break;
      await new Promise(resolve => setTimeout(resolve, 30));
    }
    assert.ok(loaded, page.slug+' must load');
    const bounds = await evaluate('({width:innerWidth,scroll:document.documentElement.scrollWidth})');
    assert.ok(bounds.scroll <= bounds.width, page.slug+' overflow '+JSON.stringify(bounds));
  }
  assert.ok(requests.every(url => url.startsWith('file:') || url.startsWith('data:')), 'all pages must load only local/embedded resources');
  // LANG02: inspect the actual generated colors and selectable source in Chrome.
  await c('Page.navigate', {url:pathToFileURL(resolve('docs/public/dist/getting-started.html')).href});
  for (let n=0;n<100;n++) {
    if (await evaluate('document.readyState === "complete" && !!document.querySelector(".syntax-keyword")')) break;
    await new Promise(resolve=>setTimeout(resolve,30));
  }
  const syntax = await evaluate(`(() => {
    const selectors=['keyword','string','comment','number','type','callable','punctuation'];
    return selectors.map(kind => {
      const token=document.querySelector('.syntax-'+kind);
      return {kind,color:token?getComputedStyle(token).color:null};
    });
  })()`);
  assert.ok(syntax.every(token=>token.color), 'all seven token classes must be rendered');
  assert.equal(new Set(syntax.map(token=>token.color)).size,7,'each token color is distinct');
  const code = await evaluate(`(() => {
    const block=document.querySelector('pre[data-example]');
    const range=document.createRange();range.selectNodeContents(block.querySelector('code'));
    const selection=getSelection();selection.removeAllRanges();selection.addRange(range);
    return {text:selection.toString(),lang:block.dataset.language,status:block.dataset.status,
      font:getComputedStyle(block).fontSize,select:getComputedStyle(block).userSelect};
  })()`);
  assert.equal(code.lang,'javascript');assert.equal(code.status,'executed');
  assert.ok(code.text.includes("import {openDatabase"),'source text remains selectable');
  assert.notEqual(code.select,'none');assert.equal(code.font,'12px');
  await evaluate('getSelection().removeAllRanges();document.querySelector("figure.example").scrollIntoView()');
  const colored = await c('Page.captureScreenshot',{format:'png'});
  await writeFile('target/public-documentation/syntax-mobile.png',Buffer.from(colored.data,'base64'));
  await c('Emulation.setDeviceMetricsOverride',{width:1440,height:1000,deviceScaleFactor:1,mobile:false});
  await evaluate('getSelection().removeAllRanges();document.querySelector("figure.example").scrollIntoView()');
  const coloredDesktop = await c('Page.captureScreenshot',{format:'png'});
  await writeFile('target/public-documentation/syntax-desktop.png',Buffer.from(coloredDesktop.data,'base64'));
  assert.ok(requests.every(url => url.startsWith('file:') || url.startsWith('data:')), 'final observed resources must be local or embedded');
  const sourceHashes = {};
  const verification = JSON.parse(await readFile('docs/public/verification.json','utf8'));
  for (const file of Object.keys(verification.source_hashes).filter(path=>path.startsWith('docs/public/')))
    sourceHashes[file] = createHash('sha256').update(await readFile(file)).digest('hex');
  for (const file of ['docs/public/assets/style.css', 'docs/public/assets/search.js', 'docs/public/site.json', ...pages.filter(p=>!['verification','sdk-declarations','native-reference','swift-reference','kotlin-reference','react-native-reference'].includes(p.slug)).map(p=>`docs/public/pages/${p.slug}.html`)]) sourceHashes[file] = createHash('sha256').update(await readFile(file)).digest('hex');
  const report = {sourceHashes, executable, syntax, selectableCode:code, resourceSchemes:[...new Set(requests.map(url=>url.split(':')[0]))], pagesCheckedAt320:pages.length, date:new Date().toISOString(), browser:await call('Browser.getVersion'), url:'file:///<checkout>/docs/public/dist/index.html', status:'passed',
    checks:['actual file:// search match/no-match/clear', 'Tab reaches visible skip link with focus outline', 'home page at 320/768/1440px; all configured pages at 320px without page overflow', 'desktop and mobile screenshots captured', 'all observed resources are local or embedded', 'seven distinct computed syntax colors; language/status labels; selectable source'], sizes,
    limitations:['Not a full screen-reader or WCAG audit.', 'No application mobile/browser storage runtime claim.']};
  await writeFile('docs/public/browser-check.json', JSON.stringify(report,null,2)+'\n');
  console.log('PASS real Chrome file:// search, keyboard focus, 320/768/1440px reflow');
} finally {
  socket?.close();
  browser.kill('SIGTERM');
  await new Promise(resolve => browser.once('exit', resolve));
  await rm(profile,{recursive:true,force:true});
}
