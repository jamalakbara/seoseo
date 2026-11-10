/** Common markets for the project picker. Any ISO 3166-1 alpha-2 code is accepted by the API. */
export const COUNTRIES: { code: string; name: string; language: string }[] = [
  { code: "US", name: "United States", language: "en" },
  { code: "GB", name: "United Kingdom", language: "en" },
  { code: "CA", name: "Canada", language: "en" },
  { code: "AU", name: "Australia", language: "en" },
  { code: "IN", name: "India", language: "en" },
  { code: "ID", name: "Indonesia", language: "id" },
  { code: "MY", name: "Malaysia", language: "ms" },
  { code: "SG", name: "Singapore", language: "en" },
  { code: "PH", name: "Philippines", language: "en" },
  { code: "DE", name: "Germany", language: "de" },
  { code: "FR", name: "France", language: "fr" },
  { code: "ES", name: "Spain", language: "es" },
  { code: "IT", name: "Italy", language: "it" },
  { code: "NL", name: "Netherlands", language: "nl" },
  { code: "BR", name: "Brazil", language: "pt" },
  { code: "MX", name: "Mexico", language: "es" },
  { code: "JP", name: "Japan", language: "ja" },
  { code: "KR", name: "South Korea", language: "ko" },
  { code: "AE", name: "United Arab Emirates", language: "ar" },
  { code: "SA", name: "Saudi Arabia", language: "ar" },
  { code: "TR", name: "Turkey", language: "tr" },
  { code: "PL", name: "Poland", language: "pl" },
  { code: "SE", name: "Sweden", language: "sv" },
  { code: "ZA", name: "South Africa", language: "en" },
  { code: "NG", name: "Nigeria", language: "en" },
];

export const LANGUAGES: { code: string; name: string }[] = [
  { code: "en", name: "English" },
  { code: "id", name: "Indonesian" },
  { code: "ms", name: "Malay" },
  { code: "de", name: "German" },
  { code: "fr", name: "French" },
  { code: "es", name: "Spanish" },
  { code: "it", name: "Italian" },
  { code: "nl", name: "Dutch" },
  { code: "pt", name: "Portuguese" },
  { code: "ja", name: "Japanese" },
  { code: "ko", name: "Korean" },
  { code: "ar", name: "Arabic" },
  { code: "tr", name: "Turkish" },
  { code: "pl", name: "Polish" },
  { code: "sv", name: "Swedish" },
];

export function countryName(code: string) {
  return COUNTRIES.find((c) => c.code === code)?.name ?? code;
}
