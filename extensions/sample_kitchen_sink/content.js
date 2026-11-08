if (window.ce && window.ce.runtime && window.ce.runtime.sendMessage) {
  window.ce.runtime.sendMessage("sample.kitchen", { title: document.title, href: location.href });
}
