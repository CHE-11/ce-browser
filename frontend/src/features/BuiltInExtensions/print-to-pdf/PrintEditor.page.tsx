import { useEffect } from "react";
import { PrintToPDFEditor } from "@/features/BuiltInExtensions/print-to-pdf/PrintToPDFEditor";

type PrintEditorPageProps = {
  params: URLSearchParams;
};

export function PrintEditorPage({ params }: PrintEditorPageProps) {
  useEffect(() => {
    document.title = "Print to PDF";
  }, []);

  return <PrintToPDFEditor tabId={Number(params.get("tab"))} />;
}
