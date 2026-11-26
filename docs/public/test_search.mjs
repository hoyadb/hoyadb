// TC-0045 DOC04: offline search logic with a minimal DOM, no network or packages.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import vm from 'node:vm';
const element = () => ({children: [], textContent: '', append(child) { this.children.push(child); }, replaceChildren() {this.children = []; this.textContent = '';}});
const input = {value: '', addEventListener(type, callback) {assert.equal(type, 'input'); this.search = callback;}};
const results = element();
const context = {document: {getElementById: id => id === 'search' ? input : results, createElement: element}, window: {HOYA_SEARCH: [{title: 'Queries', url: 'queries.html', text: 'filter order limit'}]}};
vm.runInNewContext(await readFile(new URL('./assets/search.js', import.meta.url), 'utf8'), context);
input.value = 'filter limit'; input.search();
assert.equal(results.children[0].children[0].children[0].href, 'queries.html');
input.value = 'no-match'; input.search();
assert.equal(results.textContent, 'No pages found. Try another term.');
input.value = ''; input.search(); assert.equal(results.children.length, 0);
console.log('PASS inline offline search: matching, no results, clear; no fetch/XHR');
