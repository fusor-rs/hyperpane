(() => {
  const feel = new CSSStyleSheet();
  feel.replaceSync(`
    :root {
      -webkit-tap-highlight-color: transparent;
      -webkit-touch-callout: none;
    }
  `);
  document.adoptedStyleSheets = [...document.adoptedStyleSheets, feel];
})();
