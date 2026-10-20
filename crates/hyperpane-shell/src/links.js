window.addEventListener('click', (event) => {
  if (event.defaultPrevented || !(event.target instanceof Element)) return;
  const link = event.target.closest('a[target="_blank"][href]');
  if (link === null) return;
  event.preventDefault();
  location.assign(link.href);
});
