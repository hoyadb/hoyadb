/* SPEC-0041: in-memory search, including file://; no network dependency. */
'use strict';
const input = document.getElementById('search');
const results = document.getElementById('search-results');
input.addEventListener('input', () => {
  const terms = input.value.toLowerCase().trim().split(/\s+/).filter(Boolean);
  results.replaceChildren();
  if (!terms.length) return;
  const pages = window.HOYA_SEARCH.filter(page => terms.every(term => (page.title + ' ' + page.text).toLowerCase().includes(term))).slice(0, 12);
  if (!pages.length) { results.textContent = 'No pages found. Try another term.'; return; }
  const list = document.createElement('ul');
  for (const page of pages) {
    const item = document.createElement('li');
    const link = document.createElement('a');
    link.href = page.url;
    link.textContent = page.title;
    item.append(link);
    list.append(item);
  }
  results.append(list);
});
