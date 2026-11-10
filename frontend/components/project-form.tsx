"use client";

import { useState } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { COUNTRIES, LANGUAGES } from "@/lib/markets";

const COUNTRY_ITEMS = COUNTRIES.map((c) => ({ value: c.code, label: c.name }));
const LANGUAGE_ITEMS = LANGUAGES.map((l) => ({ value: l.code, label: l.name }));

export type ProjectFormValues = {
  name: string;
  domain: string;
  brand_name: string;
  country_code: string;
  language_code: string;
};

export const emptyProject: ProjectFormValues = {
  name: "",
  domain: "",
  brand_name: "",
  country_code: "US",
  language_code: "en",
};

/** Controlled fields shared by the create dialog and the settings page. */
export function ProjectFields({
  initial,
  onChange,
}: {
  initial: ProjectFormValues;
  onChange: (v: ProjectFormValues) => void;
}) {
  const [v, setV] = useState(initial);
  const set = (patch: Partial<ProjectFormValues>) => {
    const next = { ...v, ...patch };
    setV(next);
    onChange(next);
  };

  return (
    <div className="grid gap-4">
      <div className="grid gap-2">
        <Label htmlFor="p-name">Project name</Label>
        <Input
          id="p-name"
          value={v.name}
          onChange={(e) => set({ name: e.target.value })}
          placeholder="My website"
          required
          maxLength={120}
        />
      </div>
      <div className="grid gap-2">
        <Label htmlFor="p-domain">Website</Label>
        <Input
          id="p-domain"
          value={v.domain}
          onChange={(e) => set({ domain: e.target.value })}
          placeholder="example.com"
        />
      </div>
      <div className="grid gap-2">
        <Label htmlFor="p-brand">Brand name (optional)</Label>
        <Input
          id="p-brand"
          value={v.brand_name}
          onChange={(e) => set({ brand_name: e.target.value })}
          maxLength={120}
        />
      </div>
      <div className="grid grid-cols-2 gap-4">
        <div className="grid gap-2">
          <Label>Country</Label>
          <Select
            items={COUNTRY_ITEMS}
            value={v.country_code}
            onValueChange={(code) => {
              const c = COUNTRIES.find((x) => x.code === code);
              set({ country_code: String(code), language_code: c?.language ?? v.language_code });
            }}
          >
            <SelectTrigger className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {COUNTRIES.map((c) => (
                <SelectItem key={c.code} value={c.code}>
                  {c.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="grid gap-2">
          <Label>Language</Label>
          <Select
            items={LANGUAGE_ITEMS}
            value={v.language_code}
            onValueChange={(code) => set({ language_code: String(code) })}
          >
            <SelectTrigger className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {LANGUAGES.map((l) => (
                <SelectItem key={l.code} value={l.code}>
                  {l.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>
    </div>
  );
}
