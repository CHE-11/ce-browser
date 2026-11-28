// Coupon field, apply button, order total and messages on a cart or checkout page.
// Wrapped by coupons/apply.rs in an async function that calls one of the functions below.
// It never submits a form and never clicks order, payment or checkout buttons.

var FIELD_WORDS = /coupon|promo|discount|voucher|offer.?code|redeem|reduction|rabatt|gutschein|cupon|codice|code.?promo/i;
var GIFT_WORDS = /gift.?card|giftcard|gift.?cert/i;
var FIELD_SKIP = /e-?mail|search|zip|postal|phone|tel\b|newsletter|card.?number|cc-?number|cvv|cvc|password|first.?name|last.?name|address|city|state|country|quantity|qty/i;
var APPLY_WORDS = /^\s*(apply|redeem|submit|add|use|go|ok|validate|activate|anwenden|einlösen|appliquer|aplicar|applica)\b|^\s*[›»→>]\s*$/i;
var DANGER_WORDS = /place.?order|pay\b|pay now|purchase|buy|checkout|check out|continue|complete|confirm|proceed|sign.?in|log.?in|subscribe/i;
var REMOVE_WORDS = /^\s*(remove|delete|clear|×|✕|✖|x)\s*$|remove.{0,20}(coupon|promo|code|discount|voucher)|(coupon|promo|code|discount|voucher).{0,20}remove/i;
var TOGGLE_WORDS = /(have|add|enter|apply|use|got).{0,25}(promo|coupon|discount|voucher)|^\s*(promo|coupon|discount|voucher)( or gift card)?( code)?s?\s*[+?:]?\s*$/i;
var ERROR_WORDS = /invalid|not valid|isn.t valid|is not a valid|not applicable|doesn.t apply|does not apply|cannot be applied|can.t be applied|could not be applied|couldn.t be applied|unable to apply|expired|not eligible|ineligible|not recognized|not recognised|not found|no longer|does not exist|doesn.t exist|minimum|requirements|isn.t available|not available|try again|incorrect|unknown code|already been used|can.t be used|cannot be used/i;
var SUCCESS_WORDS = /applied|you saved|you.re saving|discount added|code added|success/i;
var TOTAL_STRONG = /grand.?total|order.?total|estimated.?total|total.?due|amount.?due|you.?pay|total.?to.?pay|final.?total|total.?price|order.?summary.?total/i;
var TOTAL_PLAIN = /\btotal\b/i;
var TOTAL_SKIP = /sub.?total|savings|you.?saved|discount|tax|vat|shipping|delivery|items?.?total|points|rewards|gift/i;

function roots() {
  var out = [document];
  var walk = function (root) {
    var all = root.querySelectorAll("*");
    for (var i = 0; i < all.length; i++) {
      if (all[i].shadowRoot) {
        out.push(all[i].shadowRoot);
        walk(all[i].shadowRoot);
      }
    }
  };
  walk(document);
  var frames = document.querySelectorAll("iframe");
  for (var j = 0; j < frames.length; j++) {
    try {
      var doc = frames[j].contentDocument;
      if (doc && doc.body) {
        out.push(doc);
        walk(doc);
      }
    } catch (error) {}
  }
  return out;
}

function all(selector) {
  var out = [];
  var list = roots();
  for (var i = 0; i < list.length; i++) {
    var found = list[i].querySelectorAll(selector);
    for (var j = 0; j < found.length; j++) {
      out.push(found[j]);
    }
  }
  return out;
}

function visible(element) {
  if (!element || !element.getBoundingClientRect) {
    return false;
  }
  var rect = element.getBoundingClientRect();
  if (rect.width < 1 || rect.height < 1) {
    return false;
  }
  var view = (element.ownerDocument && element.ownerDocument.defaultView) || window;
  var style = view.getComputedStyle(element);
  return style.visibility !== "hidden" && style.display !== "none" && Number(style.opacity) > 0.05;
}

function words(element) {
  var parts = [
    element.getAttribute("name"),
    element.id,
    element.getAttribute("placeholder"),
    element.getAttribute("aria-label"),
    element.getAttribute("autocomplete"),
    element.getAttribute("data-testid"),
    element.getAttribute("data-test"),
    typeof element.className === "string" ? element.className : "",
  ];
  if (element.labels) {
    for (var i = 0; i < element.labels.length; i++) {
      parts.push(element.labels[i].innerText);
    }
  }
  var labelled = element.getAttribute("aria-labelledby");
  if (labelled) {
    var label = element.ownerDocument.getElementById(labelled);
    if (label) {
      parts.push(label.innerText);
    }
  }
  return parts.filter(Boolean).join(" ");
}

function label(element) {
  return [
    element.innerText || element.value || "",
    element.getAttribute("aria-label") || "",
    element.getAttribute("title") || "",
  ].join(" ").trim();
}

function findField() {
  var inputs = all("input:not([type=hidden]):not([type=checkbox]):not([type=radio]):not([type=submit]):not([type=button]):not([type=email]):not([type=password])");
  var best = null;
  var bestScore = 0;
  for (var i = 0; i < inputs.length; i++) {
    var input = inputs[i];
    if (input.disabled || input.readOnly) {
      continue;
    }
    var text = words(input);
    var score = 0;
    if (FIELD_WORDS.test(text)) {
      score = 4;
    } else if (GIFT_WORDS.test(text)) {
      score = 1;
    } else {
      continue;
    }
    if (FIELD_SKIP.test(text) && !/coupon|promo|voucher|discount/i.test(text)) {
      continue;
    }
    if (visible(input)) {
      score += 3;
    }
    if (score > bestScore) {
      best = input;
      bestScore = score;
    }
  }
  return best && visible(best) ? best : null;
}

function findToggle() {
  var items = all("button, a, summary, [role=button], label, span, div[tabindex], p");
  for (var i = 0; i < items.length; i++) {
    var item = items[i];
    var text = (item.innerText || "").trim();
    if (!text || text.length > 60 || !visible(item)) {
      continue;
    }
    if (TOGGLE_WORDS.test(text) && !DANGER_WORDS.test(text)) {
      return item;
    }
  }
  return null;
}

function sleep(ms) {
  return new Promise(function (resolve) {
    setTimeout(resolve, ms);
  });
}

async function field(open) {
  var input = findField();
  if (input || !open) {
    return input;
  }
  var toggle = findToggle();
  if (!toggle) {
    return null;
  }
  toggle.click();
  for (var i = 0; i < 6 && !input; i++) {
    await sleep(250);
    input = findField();
  }
  return input;
}

function nearby(input, selector, test) {
  var node = input;
  for (var depth = 0; depth < 6 && node; depth++) {
    node = node.parentElement || (node.getRootNode && node.getRootNode().host) || null;
    if (!node || !node.querySelectorAll) {
      break;
    }
    var found = node.querySelectorAll(selector);
    for (var i = 0; i < found.length; i++) {
      if (found[i] !== input && visible(found[i]) && test(found[i])) {
        return found[i];
      }
    }
  }
  return null;
}

function applyButton(input) {
  return nearby(input, "button, input[type=submit], input[type=button], [role=button], a", function (item) {
    var text = label(item);
    return APPLY_WORDS.test(text) && !DANGER_WORDS.test(text) && !REMOVE_WORDS.test(text);
  });
}

var COUPON_BOX = "[class*=coupon],[class*=Coupon],[class*=promo],[class*=Promo],[class*=voucher],[id*=coupon],[id*=promo],[id*=voucher],[data-testid*=coupon],[data-testid*=promo]";
var ITEM_PARTS = "img, input[type=number], [name*=qty], [name*=quantity], [class*=quantity], [class*=line-item], [class*=cart-item], [class*=product]";

// Only buttons that name a coupon, or a bare remove or × inside a small coupon box with no cart item in it.
function removeButtons() {
  var out = [];
  var candidates = all("button, a, [role=button]");
  for (var i = 0; i < candidates.length; i++) {
    var item = candidates[i];
    if (!visible(item)) {
      continue;
    }
    var text = label(item);
    if (!REMOVE_WORDS.test(text) || DANGER_WORDS.test(text)) {
      continue;
    }
    if (/coupon|promo|code|discount|voucher/i.test(text)) {
      out.push(item);
      continue;
    }
    var box = item.closest(COUPON_BOX);
    if (box && (box.innerText || "").length < 200 && !box.querySelector(ITEM_PARTS)) {
      out.push(item);
    }
  }
  return out;
}

function parseAmount(raw) {
  var value = raw.replace(/\s/g, "");
  var comma = value.lastIndexOf(",");
  var dot = value.lastIndexOf(".");
  if (comma > -1 && dot > -1) {
    value = comma > dot ? value.replace(/\./g, "").replace(",", ".") : value.replace(/,/g, "");
  } else if (comma > -1) {
    value = value.length - comma === 3 ? value.replace(",", ".") : value.replace(/,/g, "");
  } else if (dot > -1 && value.length - dot !== 3 && value.split(".").length > 2) {
    value = value.replace(/\./g, "");
  }
  var number = parseFloat(value);
  return isFinite(number) ? number : null;
}

var CURRENCY = { "$": "USD", "£": "GBP", "€": "EUR", "¥": "JPY", "₹": "INR" };

function money(text) {
  var clean = text.replace(/\u00a0/g, " ");
  var before = clean.match(/(-|−)?\s?([$£€¥₹]|USD|EUR|GBP|CAD|AUD|NZD|C\$|A\$)\s?(\d{1,3}(?:[,.\s]\d{3})*(?:[.,]\d{1,2})?|\d+(?:[.,]\d{1,2})?)/);
  if (before) {
    var amount = parseAmount(before[3]);
    if (amount === null || before[1]) {
      return null;
    }
    return { amount: amount, currency: CURRENCY[before[2]] || before[2].replace(/[^A-Z]/g, "") || "" };
  }
  var after = clean.match(/(\d{1,3}(?:[,.\s]\d{3})*[.,]\d{2})\s?([$£€]|USD|EUR|GBP|kr|zł)/);
  if (after) {
    var value = parseAmount(after[1]);
    return value === null ? null : { amount: value, currency: CURRENCY[after[2]] || after[2] };
  }
  return null;
}

var MONEY_COUNT = /[$£€¥₹]\s?\d|\d[.,]\d{2}\s?(USD|EUR|GBP|kr|zł|[$£€])/g;

// The price written after a "Total" label, skipping "Subtotal", "Total savings" and the like.
// A summary block lists subtotal, shipping and total together, so its first price is not the total.
function amountAfterLabel(text) {
  var label = /(sub\s*-?\s*)?(grand\s*|order\s*|estimated\s*|final\s*)?total(\s*(due|to\s*pay|price))?|amount\s*due|you\s*pay/gi;
  var found = null;
  var match;
  while ((match = label.exec(text))) {
    if (match[1]) {
      continue;
    }
    var after = text.slice(match.index + match[0].length, match.index + match[0].length + 40);
    if (/^\s*(savings|saved|discount|tax|vat|before|items?|points|weight)/i.test(after)) {
      continue;
    }
    var amount = money(after);
    if (amount) {
      found = amount;
    }
  }
  return found;
}

function readTotal() {
  var best = null;
  var items = all("*");
  for (var i = 0; i < items.length; i++) {
    var item = items[i];
    if (item.children.length > 8) {
      continue;
    }
    var text = (item.innerText || "").trim();
    if (!text || text.length > 120) {
      continue;
    }
    var hint = [text, typeof item.className === "string" ? item.className : "", item.id, item.getAttribute("data-testid") || ""].join(" ");
    var score = 0;
    if (TOTAL_STRONG.test(hint)) {
      score = 3;
    } else if (TOTAL_PLAIN.test(hint) && !TOTAL_SKIP.test(hint)) {
      score = 2;
    } else if (TOTAL_PLAIN.test(text) && amountAfterLabel(text)) {
      score = 2;
    } else {
      continue;
    }
    var prices = (text.match(MONEY_COUNT) || []).length;
    var found = amountAfterLabel(text) || (prices === 1 ? money(text) : null);
    if (!found || !visible(item)) {
      continue;
    }
    if (!best || score > best.score || (score === best.score && text.length < best.length)) {
      best = { score: score, length: text.length, amount: found.amount, currency: found.currency };
    }
  }
  return best;
}

function message(pattern) {
  var items = all("[role=alert], [aria-live], [class*=error], [class*=Error], [class*=invalid], [class*=message], [class*=Message], [class*=notice], [class*=feedback], [class*=warning], [class*=alert], [id*=error], [id*=message], small, p, span, div");
  for (var i = 0; i < items.length; i++) {
    var item = items[i];
    if (item.children.length > 4) {
      continue;
    }
    var text = (item.innerText || "").trim();
    if (!text || text.length > 200 || !pattern.test(text) || !visible(item)) {
      continue;
    }
    return text;
  }
  return "";
}

function state(input) {
  var total = readTotal();
  return {
    found: Boolean(input),
    toggle: !input && Boolean(findToggle()),
    total: total ? total.amount : null,
    currency: total ? total.currency : "",
    error: message(ERROR_WORDS),
    success: message(SUCCESS_WORDS),
    value: input ? input.value : "",
  };
}

async function detect(open) {
  return state(await field(open));
}

async function removeApplied() {
  var buttons = removeButtons();
  for (var i = 0; i < buttons.length; i++) {
    buttons[i].click();
    await sleep(150);
  }
  return buttons.length;
}

function setValue(input, value) {
  var view = (input.ownerDocument && input.ownerDocument.defaultView) || window;
  var setter = Object.getOwnPropertyDescriptor(view.HTMLInputElement.prototype, "value").set;
  input.focus();
  setter.call(input, "");
  input.dispatchEvent(new view.Event("input", { bubbles: true }));
  setter.call(input, value);
  input.dispatchEvent(new view.InputEvent("input", { bubbles: true, data: value, inputType: "insertText" }));
  input.dispatchEvent(new view.Event("change", { bubbles: true }));
}

function pressEnter(input) {
  var view = (input.ownerDocument && input.ownerDocument.defaultView) || window;
  var options = { key: "Enter", code: "Enter", keyCode: 13, which: 13, bubbles: true, cancelable: true };
  input.dispatchEvent(new view.KeyboardEvent("keydown", options));
  input.dispatchEvent(new view.KeyboardEvent("keypress", options));
  input.dispatchEvent(new view.KeyboardEvent("keyup", options));
}

async function applyCode(code) {
  var input = await field(true);
  if (!input) {
    return { ok: false, reason: "No coupon field on this page." };
  }
  setValue(input, code);
  await sleep(200);
  var button = applyButton(input);
  for (var i = 0; i < 5 && button && button.disabled; i++) {
    await sleep(150);
  }
  if (button && !button.disabled) {
    button.click();
  } else {
    pressEnter(input);
  }
  input.blur();
  return { ok: true, reason: button ? "button" : "enter" };
}
