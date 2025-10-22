import type { AppProps } from "next/app";
import { ConfigProvider, theme } from "antd";
import zhCN from "antd/locale/zh_CN";
import "antd/dist/reset.css";
import "react-diff-view/style/index.css";
import "../styles/globals.css";
import { AuthProvider } from "@/contexts/AuthContext";

export default function CodexApp({ Component, pageProps }: AppProps) {
  return (
    <ConfigProvider
      locale={zhCN}
      theme={{
        token: {
          colorPrimary: "#2563eb"
        },
        algorithm: [theme.darkAlgorithm]
      }}
    >
      <AuthProvider>
        <Component {...pageProps} />
      </AuthProvider>
    </ConfigProvider>
  );
}
