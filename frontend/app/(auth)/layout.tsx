export default function AuthLayout({ children }: LayoutProps<"/">) {
  return (
    <main className="flex min-h-screen flex-1 items-center justify-center bg-muted/40 p-4">
      <div className="w-full max-w-sm">
        <div className="mb-6 text-center">
          <div className="text-2xl font-semibold tracking-tight">SeoSeo</div>
          <p className="text-sm text-muted-foreground">Free, open-source SEO platform</p>
        </div>
        {children}
      </div>
    </main>
  );
}
