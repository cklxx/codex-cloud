"use client";

import { PropsWithChildren, useEffect } from "react";
import { useRouter } from "next/router";
import { Spin } from "antd";
import { useAuth } from "@/contexts/AuthContext";

export const AuthGuard: React.FC<PropsWithChildren> = ({ children }) => {
  const router = useRouter();
  const { token, loading } = useAuth();

  useEffect(() => {
    if (!loading && !token) {
      void router.replace("/");
    }
  }, [token, loading, router]);

  if (loading || !token) {
    return (
      <div style={{ display: "flex", alignItems: "center", justifyContent: "center", minHeight: "100vh" }}>
        <Spin tip="加载中" size="large" />
      </div>
    );
  }

  return <>{children}</>;
};
