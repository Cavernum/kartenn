import { existsSync, readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { createI18n } from "vue-i18n";
import en from "./en.json";
import fr from "./fr.json";
import { chooseLocale, problemText } from "./index.ts";

type Messages = { [key: string]: string | Messages };

function keys(messages: Messages, prefix = ""): string[] {
  return Object.entries(messages).flatMap(([key, value]) =>
    typeof value === "string" ? [`${prefix}${key}`] : keys(value, `${prefix}${key}.`),
  );
}

describe("catalogs (REQ-007-29)", () => {
  it("the shell has the same keys in French and English", () => {
    expect(keys(fr).sort()).toEqual(keys(en).sort());
  });

  it("every first-party plugin has the same keys in French and English", () => {
    const plugins = fileURLToPath(new URL("../../../plugins/", import.meta.url));
    for (const plugin of readdirSync(plugins)) {
      const dir = `${plugins}${plugin}/i18n`;
      if (!existsSync(`${dir}/fr.json`) && !existsSync(`${dir}/en.json`)) continue;
      const read = (locale: string) =>
        keys(JSON.parse(readFileSync(`${dir}/${locale}.json`, "utf8")) as Messages).sort();
      expect(read("fr"), plugin).toEqual(read("en"));
    }
  });

  it("every message compiles", () => {
    const i18n = createI18n({ legacy: false, locale: "fr", messages: { fr, en } });
    for (const locale of ["fr", "en"] as const) {
      i18n.global.locale.value = locale;
      for (const key of keys(fr)) expect(i18n.global.t(key, { zone: "UTC" }), key).not.toBe(key);
    }
  });
});

describe("the language (REQ-007-29)", () => {
  it("is the one kept first", () => {
    expect(chooseLocale("fr", ["en-US"])).toBe("fr");
    expect(chooseLocale("en", ["fr-FR"])).toBe("en");
  });

  it("then the first language of the browser that is offered, by its prefix", () => {
    expect(chooseLocale(null, ["de-DE", "fr-CA", "en"])).toBe("fr");
    expect(chooseLocale("de", ["EN-gb", "fr"])).toBe("en");
  });

  it("else English", () => {
    expect(chooseLocale(null, [])).toBe("en");
    expect(chooseLocale(null, ["de", "it"])).toBe("en");
  });
});

describe("problems of the API (REQ-007-29)", () => {
  const i18n = createI18n({ legacy: false, locale: "fr", messages: { fr, en } }).global;

  it("are translated by their type", () => {
    expect(
      problemText(i18n, { type: "urn:kartenn:problem:forbidden", title: "Forbidden", detail: "x" }),
    ).toBe(fr.problem.forbidden);
  });

  it("without a translation of their type, show their detail, else their title", () => {
    expect(
      problemText(i18n, { type: "urn:kartenn:problem:unheard-of", title: "T", detail: "raw" }),
    ).toBe("raw");
    expect(problemText(i18n, { type: "about:blank", title: "Not Found", detail: null })).toBe(
      "Not Found",
    );
  });
});
