"use client";
import { Button } from "@/components/ui/button";
import { useLocale } from "@/lib/locale";
export function Result({ value }: { value: unknown }) {
  const { t } = useLocale();
  const json = JSON.stringify(value, null, 2);
  return (
    <section className="grid min-w-0 gap-3" aria-live="polite">
      <div className="flex items-center justify-between">
        <h3 className="font-semibold">{t("Result")}</h3>
        <Button
          variant="outline"
          size="sm"
          onClick={() => {
            const url = URL.createObjectURL(
              new Blob([json], { type: "application/json" }),
            );
            const a = document.createElement("a");
            a.href = url;
            a.download = "seoseo-evidence.json";
            a.click();
            setTimeout(() => URL.revokeObjectURL(url), 1000);
          }}
        >
          {t("Download JSON")}
        </Button>
      </div>
      <pre className="max-h-96 overflow-auto whitespace-pre-wrap break-all rounded-lg border bg-muted/30 p-4 text-xs">
        {json}
      </pre>
    </section>
  );
}
export async function request(path: string, method = "GET", body?: unknown) {
  const r = await fetch(`/api/v1${path}`, {
    method,
    headers: { "content-type": "application/json", "x-seoseo-csrf": "1" },
    body: body === undefined ? undefined : JSON.stringify(body),
    cache: "no-store",
  });
  if (r.status === 204) return null;
  const v = await r.json();
  if (!r.ok) throw new Error(v.detail ?? `Request failed (${r.status})`);
  return v;
}
