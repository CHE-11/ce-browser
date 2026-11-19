import { useEffect } from "react";
import { PrintToPDFEditor } from "@/built_in_extensions/print-to-pdf/PrintToPDFEditor";

type PrintEditorPageProps = {
  params: URLSearchParams;
};

export function PrintEditorPage({ params }: PrintEditorPageProps) {
  useEffect(() => {
    document.title = "Print to PDF";
  }, []);

  return <PrintToPDFEditor tabId={Number(params.get("tab"))} />;
}
