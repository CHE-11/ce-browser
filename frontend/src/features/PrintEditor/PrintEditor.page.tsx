import { useEffect } from "react";
import { PrintEditor } from "@features/BuiltInExtensions/print-pdf/PrintEditor";

type PrintEditorPageProps = {
  params: URLSearchParams;
};

export function PrintEditorPage({ params }: PrintEditorPageProps) {
  useEffect(() => {
    document.title = "Print to PDF";
  }, []);

  return <PrintEditor tabId={Number(params.get("tab"))} />;
}
