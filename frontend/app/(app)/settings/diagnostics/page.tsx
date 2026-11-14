"use client";
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { Result, request } from "@/components/extras/result";
type View = {
  enabled: boolean;
  receiver_configured: boolean;
  payload: unknown;
  note: string;
};
export default function DiagnosticsPage() {
  const data = useQuery({
    queryKey: ["telemetry"],
    queryFn: () => request("/settings/telemetry") as Promise<View>,
  });
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [sent, setSent] = useState(false);
  async function change(send: boolean) {
    setBusy(true);
    setError("");
    setSent(false);
    try {
      await request(
        `/settings/telemetry${send ? "/send" : ""}`,
        send ? "POST" : "PUT",
        send ? undefined : { enabled: !data.data?.enabled },
      );
      if (send) setSent(true);
      await data.refetch();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="grid max-w-2xl gap-5">
      <h1 className="text-2xl font-semibold">Optional diagnostics</h1>
      {(error || data.error) && (
        <p role="alert" className="text-destructive">
          {error || data.error?.message}
        </p>
      )}
      {data.data && (
        <>
          <p className="text-sm text-muted-foreground">{data.data.note}</p>
          <p>
            Status: {data.data.enabled ? "Opted in" : "Off"} · Receiver:{" "}
            {data.data.receiver_configured
              ? "Configured by operator"
              : "Not configured"}
          </p>
          <Result value={data.data.payload} />
          <div className="flex flex-wrap gap-3">
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => void change(false)}
            >
              {data.data.enabled
                ? "Turn off diagnostics"
                : "Opt in to diagnostics"}
            </Button>
            <Button
              disabled={
                busy || !data.data.enabled || !data.data.receiver_configured
              }
              onClick={() => void change(true)}
            >
              Send this payload once
            </Button>
          </div>
          {sent && <p role="status">Diagnostic payload delivered.</p>}
        </>
      )}
    </div>
  );
}
