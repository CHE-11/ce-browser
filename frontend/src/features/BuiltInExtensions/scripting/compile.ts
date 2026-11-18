import type { ScriptLanguage } from "@type/browser";

/** JavaScript or CSS to store. Python stays empty. */
export async function compileSource(language: ScriptLanguage, source: string) {
  if (language === "python") {
    return "";
  }
  if (language === "javascript" || language === "css") {
    return source;
  }
  const ts = await import("typescript");
  const output = ts.transpileModule(source, {
    compilerOptions: {
      target: ts.ScriptTarget.ES2022,
      module: ts.ModuleKind.None,
      strict: false,
    },
    reportDiagnostics: false,
  });
  return output.outputText;
}
