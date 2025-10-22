"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { message } from "antd";
import { apiFetch } from "@/lib/api";

type AuthContextValue = {
  token: string | null;
  loading: boolean;
  login: (email: string, password: string) => Promise<void>;
  register: (email: string, password: string, name?: string) => Promise<void>;
  logout: () => void;
};

const AuthContext = createContext<AuthContextValue | undefined>(undefined);

const STORAGE_KEY = "codex-cloud-token";

export const AuthProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [token, setToken] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored) {
      setToken(stored);
    }
    setLoading(false);
  }, []);

  const login = useCallback(async (email: string, password: string) => {
    const response = await apiFetch("/auth/session", {
      method: "POST",
      body: JSON.stringify({ email, password })
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(errorText || "登录失败");
    }

    const data = (await response.json()) as { access_token: string };
    setToken(data.access_token);
    if (typeof window !== "undefined") {
      window.localStorage.setItem(STORAGE_KEY, data.access_token);
    }
    message.success("登录成功");
  }, []);

  const register = useCallback(async (email: string, password: string, name?: string) => {
    const response = await apiFetch("/auth/users", {
      method: "POST",
      body: JSON.stringify({ email, password, name })
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(errorText || "注册失败");
    }

    message.success("注册成功，请登录");
  }, []);

  const logout = useCallback(() => {
    setToken(null);
    if (typeof window !== "undefined") {
      window.localStorage.removeItem(STORAGE_KEY);
    }
  }, []);

  const value = useMemo(
    () => ({
      token,
      loading,
      login,
      register,
      logout
    }),
    [token, loading, login, register, logout]
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
};

export const useAuth = (): AuthContextValue => {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error("useAuth must be used within AuthProvider");
  }
  return ctx;
};
