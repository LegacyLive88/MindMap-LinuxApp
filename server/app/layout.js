import "./globals.css";

export const metadata = {
  title: "MindMap cloud",
  description: "Sign in, keep backups, and edit the mind map from the browser.",
  robots: { index: false, follow: false },
};

export default function RootLayout({ children }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
