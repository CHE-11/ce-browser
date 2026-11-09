function write(name, value) {
  try {
    document.documentElement.setAttribute(name, value);
  } catch (error) {
    /* The page can refuse the attribute. The popup still records the other probes. */
  }
}

write("data-ce-content", "1");

try {
  chrome.tabs.query({}, function (tabs) {
    write("data-ce-tabs", String((tabs || []).length));
  });
  chrome.windows.getAll({}, function (windows) {
    write("data-ce-windows", String((windows || []).length));
  });
} catch (error) {
  write("data-ce-tabs", String(error && error.message ? error.message : error));
}
