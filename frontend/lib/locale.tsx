"use client";

import { useEffect, useSyncExternalStore } from "react";
const translations: Record<string, string> = {
  Projects: "Proyek",
  Team: "Tim",
  "Data providers": "Penyedia data",
  Jobs: "Pekerjaan",
  "Connected apps": "Aplikasi terhubung",
  "API keys": "Kunci API",
  Account: "Akun",
  Overview: "Ringkasan",
  Keywords: "Kata kunci",
  "Rank tracking": "Pelacakan peringkat",
  "Local SEO": "SEO lokal",
  Competitive: "Persaingan",
  Backlinks: "Tautan balik",
  "AI visibility": "Visibilitas AI",
  "Search Console": "Search Console",
  "Site audit": "Audit situs",
  Memory: "Memori",
  Assistant: "Asisten",
  Alerts: "Peringatan",
  Business: "Bisnis",
  Reports: "Laporan",
  Integrations: "Integrasi",
  Settings: "Pengaturan",
  Extras: "Alat tambahan",
  "Sign out": "Keluar",
  "Public tools": "Alat publik",
  Documentation: "Dokumentasi",
  "Free SEO tools": "Alat SEO gratis",
  "No account or paid key required.": "Tanpa akun atau kunci berbayar.",
  Tool: "Alat",
  Content: "Konten",
  "Public URL": "URL publik",
  Analyze: "Analisis",
  "Analyzing…": "Menganalisis…",
  Result: "Hasil",
  "Download JSON": "Unduh JSON",
  "Keyword generator": "Generator kata kunci",
  "SERP preview": "Pratinjau SERP",
  "Backlink checker": "Pemeriksa tautan balik",
  "Authority checker": "Pemeriksa otoritas",
  "Domain age": "Usia domain",
  "Robots tester": "Penguji robots",
  "Sitemap validator": "Validator sitemap",
  "Meta checker": "Pemeriksa meta",
  "Schema checker": "Pemeriksa schema",
  "Redirect checker": "Pemeriksa pengalihan",
  "Hreflang checker": "Pemeriksa hreflang",
  "Log analyzer": "Analisis log",
  "Measured volatility": "Volatilitas terukur",
  "Crawler access": "Akses perayap",
  "Advertising metrics": "Metrik iklan",
  "Save credential": "Simpan kredensial",
  Remove: "Hapus",
  "Refresh evidence": "Perbarui bukti",
  "Analyze log": "Analisis log",
  Diagnostics: "Diagnostik",
};
const subscribe = (callback: () => void) => {
  window.addEventListener("storage", callback);
  window.addEventListener("seoseo-locale", callback);
  return () => {
    window.removeEventListener("storage", callback);
    window.removeEventListener("seoseo-locale", callback);
  };
};
const snapshot = () => {
  try {
    return localStorage.getItem("seoseo-locale") === "id" ? "id" : "en";
  } catch {
    return "en";
  }
};
export function useLocale() {
  const locale = useSyncExternalStore(subscribe, snapshot, () => "en");
  return {
    locale,
    t: (text: string) =>
      locale === "id" ? (translations[text] ?? text) : text,
  };
}
export function LocalePicker() {
  const { locale } = useLocale();
  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);
  return (
    <label className="flex items-center gap-2 text-sm">
      <span className="sr-only">Navigation language / Bahasa navigasi</span>
      <select
        aria-label="Navigation language / Bahasa navigasi"
        className="rounded border bg-background p-1.5"
        value={locale}
        onChange={(e) => {
          try {
            localStorage.setItem("seoseo-locale", e.target.value);
          } catch {
            /* session preference remains optional */
          }
          window.dispatchEvent(new Event("seoseo-locale"));
        }}
      >
        <option value="en">English</option>
        <option value="id">Bahasa Indonesia</option>
      </select>
    </label>
  );
}
