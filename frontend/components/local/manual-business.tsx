"use client";

import type { Schemas } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

export function ManualBusiness({ onSelect }: { onSelect: (business: Schemas["LocalBusiness"]) => void }) {
  return <details><summary className="cursor-pointer text-sm font-medium">Enter business details manually</summary>
    <form className="mt-3 grid gap-3 sm:grid-cols-2" onSubmit={e => {
      e.preventDefault();
      const data = new FormData(e.currentTarget);
      onSelect({
        source: "manual", source_id: `manual:${crypto.randomUUID()}`,
        name: String(data.get("name")).trim(), address: String(data.get("address")).trim(),
        latitude: Number(data.get("latitude")), longitude: Number(data.get("longitude")),
        category: null, website: null, source_url: "",
      });
    }}>
      <p className="text-sm text-muted-foreground sm:col-span-2">Use a name, address and coordinates you have verified. OpenStreetMap coverage is incomplete. This step runs no external lookup or rank measurement.</p>
      <label className="grid gap-1 text-sm">Manual business name<Input name="name" required maxLength={200} /></label>
      <label className="grid gap-1 text-sm">Manual business address<Input name="address" required maxLength={500} /></label>
      <label className="grid gap-1 text-sm">Business latitude<Input name="latitude" type="number" step="any" min={-85} max={85} required /></label>
      <label className="grid gap-1 text-sm">Business longitude<Input name="longitude" type="number" step="any" min={-180} max={180} required /></label>
      <Button type="submit">Use manual business</Button>
    </form>
  </details>;
}
