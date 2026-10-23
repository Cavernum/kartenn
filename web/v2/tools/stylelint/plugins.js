// Rules of kartenn for Stylelint (spec 020, design « Architecture CSS »; REQ-020-12, 13):
// written with the plugin API of Stylelint, cheap to replace.
import stylelint from "stylelint";

const {
  createPlugin,
  utils: { report, ruleMessages, validateOptions },
} = stylelint;

/** The order of the layers, declared once by `styles/layers.css` (REQ-020-12). */
export const LAYERS = ["reset", "tokens", "base", "components", "plugins", "utilities"];

/** Name of the layer a node sits in, or null outside any `@layer` block. */
function layerOf(node) {
  for (let parent = node.parent; parent; parent = parent.parent) {
    if (parent.type === "atrule" && parent.name === "layer") return parent.params.trim();
  }
  return null;
}

// kartenn/known-layer: every rule and at-rule sits in a block of a known layer; the
// statement `@layer a, b;` names known layers only.
const knownLayerName = "kartenn/known-layer";
const knownLayerMessages = ruleMessages(knownLayerName, {
  outside: (what) => `${what} outside a layer: put it in @layer ${LAYERS.join(" | ")}`,
  unknown: (name) => `unknown layer « ${name} » (known: ${LAYERS.join(", ")})`,
});
const knownLayer = createPlugin(knownLayerName, (primary) => (root, result) => {
  if (!validateOptions(result, knownLayerName, { actual: primary, possible: [true] })) return;
  root.walkAtRules("layer", (atRule) => {
    for (const name of atRule.params.split(",").map((part) => part.trim())) {
      if (name && !LAYERS.includes(name.split(".")[0])) {
        report({
          ruleName: knownLayerName,
          result,
          node: atRule,
          message: knownLayerMessages.unknown(name),
        });
      }
    }
  });
  const check = (node, what) => {
    if (layerOf(node) === null) {
      report({ ruleName: knownLayerName, result, node, message: knownLayerMessages.outside(what) });
    }
  };
  root.each((node) => {
    if (node.type === "rule") check(node, `rule « ${node.selector} »`);
    else if (node.type === "atrule" && !["layer", "import", "charset"].includes(node.name)) {
      check(node, `@${node.name}`);
    } else if (node.type === "atrule" && node.name === "import" && !/\blayer\(/.test(node.params)) {
      // An imported sheet must name its layer, unless it only declares layers itself.
      if (!/layers\.css/.test(node.params)) check(node, "@import without layer()");
    }
  });
});

// kartenn/token-reference: a custom property read outside the token file is one of
// kartenn (`--k-…`), never a private variable that would carry a raw value.
const tokenReferenceName = "kartenn/token-reference";
const tokenReferenceMessages = ruleMessages(tokenReferenceName, {
  foreign: (name) => `var(${name}): only tokens --k-… are read (REQ-020-4)`,
  defined: (name) =>
    `${name} defined outside the token file: dynamic values only, set by style.setProperty`,
});
const tokenReference = createPlugin(tokenReferenceName, (primary) => (root, result) => {
  if (!validateOptions(result, tokenReferenceName, { actual: primary, possible: [true] })) return;
  root.walkDecls((decl) => {
    for (const [, name] of decl.value.matchAll(/var\(\s*(--[\w-]+)/g)) {
      if (!name.startsWith("--k-")) {
        report({
          ruleName: tokenReferenceName,
          result,
          node: decl,
          message: tokenReferenceMessages.foreign(name),
        });
      }
    }
    // A component may declare a custom property only from a token (a local alias),
    // never from a raw value.
    if (decl.prop.startsWith("--") && !/^\s*var\(--k-[\w-]+\)\s*$/.test(decl.value)) {
      report({
        ruleName: tokenReferenceName,
        result,
        node: decl,
        message: tokenReferenceMessages.defined(decl.prop),
      });
    }
  });
});

// kartenn/raw-length: no length, duration or angle written out, inside functions
// (`calc()`, `min()`…) included, which declaration-strict-value leaves aside; `1px` only as
// a border width, 0 and percentages always (REQ-020-13).
const rawLengthName = "kartenn/raw-length";
const rawLengthMessages = ruleMessages(rawLengthName, {
  raw: (value, prop) => `raw « ${value} » in ${prop}: use a token var(--k-…) (REQ-020-13)`,
});
const BORDER_WIDTH =
  /^(border(-(block|inline|top|right|bottom|left)(-(start|end))?)?(-width)?|outline(-width)?)$/;
const DIMENSION =
  /(?<![\w-])(-?\d*\.?\d+)(px|rem|em|ex|ch|lh|rlh|vw|vh|vmin|vmax|vi|vb|svh|lvh|dvh|cqi|cqb|pt|pc|cm|mm|in|q|ms|s|deg|rad|turn|fr)(?![\w-])/gi;
const rawLength = createPlugin(rawLengthName, (primary) => (root, result) => {
  if (!validateOptions(result, rawLengthName, { actual: primary, possible: [true] })) return;
  root.walkDecls((decl) => {
    if (decl.prop.startsWith("--")) return;
    // Tokens may sit inside a function: only what is written out is checked.
    const value = decl.value.replace(/var\([^)]*\)/g, "");
    for (const [match, number, unit] of value.matchAll(DIMENSION)) {
      if (Number(number) === 0) continue;
      if (match === "1px" && BORDER_WIDTH.test(decl.prop)) continue;
      if (unit.toLowerCase() === "fr" && /^grid-template/.test(decl.prop)) continue;
      report({
        ruleName: rawLengthName,
        result,
        node: decl,
        message: rawLengthMessages.raw(match, decl.prop),
      });
    }
  });
});

// kartenn/floating-shadow: elevation belongs to floating layers only (REQ-020-9): the
// shadows of overlays and modals in the sheets of the closed list, the inner edge of a
// selection or a draft anywhere.
const floatingShadowName = "kartenn/floating-shadow";
const floatingShadowMessages = ruleMessages(floatingShadowName, {
  rejected: (value) =>
    `box-shadow « ${value} »: only var(--k-shadow-inset-…) here; overlay and modal shadows ` +
    "belong to the floating components (stylelint.config.js, FLOATING)",
});
const floatingShadow = createPlugin(floatingShadowName, (primary, options) => (root, result) => {
  if (!validateOptions(result, floatingShadowName, { actual: primary, possible: [true] })) {
    return;
  }
  const file = root.source?.input.file ?? "";
  const floating = (options?.floating ?? []).some((name) =>
    new RegExp(`/components/${name}/`).test(file),
  );
  root.walkDecls(/^box-shadow$/, (decl) => {
    const value = decl.value.trim();
    if (value === "none") return;
    const inset = /^var\(--k-shadow-inset-[\w-]+\)$/.test(value);
    const layer = /^var\(--k-shadow-(overlay|modal)\)$/.test(value);
    if (inset || (layer && floating)) return;
    report({
      ruleName: floatingShadowName,
      result,
      node: decl,
      message: floatingShadowMessages.rejected(value),
    });
  });
});

export default [knownLayer, tokenReference, rawLength, floatingShadow];
