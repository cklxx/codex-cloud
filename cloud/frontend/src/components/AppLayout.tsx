"use client";

import { Layout, Menu, Typography } from "antd";
import Link from "next/link";
import { useRouter } from "next/router";
import { useMemo } from "react";
import { useAuth } from "@/contexts/AuthContext";

const { Header, Content } = Layout;

export const AppLayout: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const router = useRouter();
  const { logout } = useAuth();

  const selectedKey = useMemo(() => {
    if (router.pathname.startsWith("/tasks/create")) {
      return "create";
    }
    if (router.pathname.startsWith("/tasks")) {
      return "tasks";
    }
    return "home";
  }, [router.pathname]);

  return (
    <Layout className="main-layout">
      <Header
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center"
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 16 }}>
          <Typography.Title level={4} style={{ color: "white", margin: 0 }}>
            Codex Cloud
          </Typography.Title>
          <Menu
            theme="dark"
            mode="horizontal"
            selectedKeys={[selectedKey]}
            items={[
              { key: "tasks", label: <Link href="/tasks">任务列表</Link> },
              { key: "create", label: <Link href="/tasks/create">创建任务</Link> }
            ]}
          />
        </div>
        <Typography.Link style={{ color: "white" }} onClick={logout}>
          退出登录
        </Typography.Link>
      </Header>
      <Content style={{ padding: 24 }}>{children}</Content>
    </Layout>
  );
};
