"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { useRouter } from "next/router";
import {
  Alert,
  Badge,
  Button,
  Card,
  Descriptions,
  Divider,
  Form,
  Input,
  List,
  Radio,
  Result,
  Space,
  Tag,
  Typography,
  message
} from "antd";
import dayjs from "dayjs";
import { AuthGuard } from "@/components/AuthGuard";
import { AppLayout } from "@/components/AppLayout";
import ArtifactModal, { ArtifactViewerType } from "@/components/ArtifactModal";
import { useAuth } from "@/contexts/AuthContext";
import { apiFetchAuthed } from "@/lib/api";

interface TaskDetail {
  id: string;
  title: string;
  description?: string | null;
  status: string;
  repository_id: string;
  updated_at: string;
  assignee_id?: string | null;
  attempts: Attempt[];
}

interface Attempt {
  id: string;
  status: string;
  diff_artifact_id?: string | null;
  log_artifact_id?: string | null;
  updated_at: string;
}

const statusColors: Record<string, string> = {
  pending: "default",
  claimed: "processing",
  running: "warning",
  review: "success",
  applied: "magenta"
};

const attemptStatus: Record<string, string> = {
  queued: "排队",
  running: "执行中",
  succeeded: "成功",
  failed: "失败"
};

export default function TaskDetailPage() {
  const router = useRouter();
  const { id } = router.query;
  const { token } = useAuth();
  const [detail, setDetail] = useState<TaskDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [artifactContent, setArtifactContent] = useState<string | null>(null);
  const [artifactTitle, setArtifactTitle] = useState<string>("");
  const [modalVisible, setModalVisible] = useState(false);
  const [artifactType, setArtifactType] = useState<ArtifactViewerType | null>(null);
  const [completing, setCompleting] = useState(false);
  const [claiming, setClaiming] = useState(false);
  const [creatingAttempt, setCreatingAttempt] = useState(false);
  const [artifactLoading, setArtifactLoading] = useState(false);
  const [artifactError, setArtifactError] = useState<string | null>(null);

  const fetchDetail = useCallback(async () => {
    if (!token || typeof id !== "string") return;
    setLoading(true);
    try {
      const response = await apiFetchAuthed(`/tasks/${id}`, token);
      const data = (await response.json()) as TaskDetail;
      setDetail(data);
    } catch (error) {
      console.error(error);
      message.error("加载任务详情失败");
    } finally {
      setLoading(false);
    }
  }, [id, token]);

  useEffect(() => {
    void fetchDetail();
  }, [fetchDetail]);

  const openArtifact = useCallback(
    async (
      artifactId: string | null | undefined,
      title: string,
      type: ArtifactViewerType
    ) => {
      if (!artifactId || !token) return;
      setArtifactError(null);
      setArtifactContent(null);
      setArtifactTitle(title);
      setArtifactType(type);
      setModalVisible(true);
      setArtifactLoading(true);
      try {
        const response = await apiFetchAuthed(`/artifacts/${artifactId}`, token, {
          method: "GET"
        });
        const text = await response.text();
        setArtifactContent(text);
      } catch (error) {
        console.error(error);
        setArtifactError("加载附件失败");
        message.error("加载附件失败");
      } finally {
        setArtifactLoading(false);
      }
    },
    [token]
  );

  const closeArtifactModal = useCallback(() => {
    setModalVisible(false);
    setArtifactLoading(false);
    setArtifactContent(null);
    setArtifactError(null);
    setArtifactType(null);
  }, []);

  const claimTask = async () => {
    if (!token || typeof id !== "string") return;
    try {
      setClaiming(true);
      await apiFetchAuthed(`/tasks/${id}/claim`, token, { method: "POST" });
      await fetchDetail();
    } catch (error) {
      console.error(error);
      message.error("认领任务失败");
    } finally {
      setClaiming(false);
    }
  };

  const createAttempt = async () => {
    if (!token || typeof id !== "string") return;
    try {
      setCreatingAttempt(true);
      await apiFetchAuthed(`/tasks/${id}/attempts`, token, {
        method: "POST",
        body: JSON.stringify({ environment_id: "default" })
      });
      await fetchDetail();
    } catch (error) {
      console.error(error);
      message.error("创建尝试失败，请确认任务已被你认领");
    } finally {
      setCreatingAttempt(false);
    }
  };

  const latestRunningAttempt = useMemo(() => {
    if (!detail) return undefined;
    return detail.attempts.find((attempt) => attempt.status === "running");
  }, [detail]);

  const completeAttempt = async (values: { status: string; diff?: string; log?: string }) => {
    if (!token || !latestRunningAttempt) return;
    try {
      setCompleting(true);
      await apiFetchAuthed(`/tasks/attempts/${latestRunningAttempt.id}/complete`, token, {
        method: "POST",
        body: JSON.stringify({
          status: values.status,
          diff: values.diff,
          log: values.log
        })
      });
      await fetchDetail();
      message.success("尝试结果已提交");
    } catch (error) {
      console.error(error);
      message.error("提交尝试结果失败");
    } finally {
      setCompleting(false);
    }
  };

  if (!id) {
    return null;
  }

  return (
    <AuthGuard>
      <AppLayout>
        <Card loading={loading}>
          {detail ? (
            <>
              <Typography.Title level={4}>{detail.title}</Typography.Title>
              <Descriptions column={1} bordered>
                <Descriptions.Item label="任务状态">
                  <Tag color={statusColors[detail.status] ?? "default"}>{detail.status}</Tag>
                </Descriptions.Item>
                <Descriptions.Item label="仓库">{detail.repository_id}</Descriptions.Item>
                <Descriptions.Item label="最近更新">
                  {dayjs(detail.updated_at).format("YYYY-MM-DD HH:mm:ss")}
                </Descriptions.Item>
                <Descriptions.Item label="认领人">
                  {detail.assignee_id ? (
                    <Badge color="#22d3ee" text={detail.assignee_id} />
                  ) : (
                    <span>未认领</span>
                  )}
                </Descriptions.Item>
                <Descriptions.Item label="描述">
                  {detail.description ?? "无"}
                </Descriptions.Item>
              </Descriptions>

              <Space style={{ marginTop: 16 }}>
                <Button type="primary" onClick={() => void fetchDetail()}>
                  刷新
                </Button>
                <Button type="default" loading={claiming} onClick={() => void claimTask()}>
                  认领任务
                </Button>
                <Button
                  type="dashed"
                  loading={creatingAttempt}
                  disabled={detail.status !== "claimed"}
                  onClick={() => void createAttempt()}
                >
                  启动尝试
                </Button>
              </Space>

              <Divider orientation="left">执行尝试</Divider>
              {detail.attempts.length === 0 ? (
                <Alert message="暂无尝试" type="info" showIcon />
              ) : (
                <List
                  bordered
                  dataSource={detail.attempts}
                  renderItem={(attempt) => (
                    <List.Item
                      actions={[
                        attempt.diff_artifact_id ? (
                          <Button
                            key="diff"
                            type="link"
                            onClick={() =>
                              void openArtifact(attempt.diff_artifact_id, "Diff 详情", "diff")
                            }
                          >
                            查看 Diff
                          </Button>
                        ) : null,
                        attempt.log_artifact_id ? (
                          <Button
                            key="log"
                            type="link"
                            onClick={() =>
                              void openArtifact(attempt.log_artifact_id, "执行日志", "log")
                            }
                          >
                            查看日志
                          </Button>
                        ) : null
                      ].filter(Boolean)}
                    >
                      <List.Item.Meta
                        title={
                          <Space>
                            <Typography.Text strong>#{attempt.id.slice(0, 8)}</Typography.Text>
                            <Tag color="blue">{attemptStatus[attempt.status] ?? attempt.status}</Tag>
                          </Space>
                        }
                        description={`更新于 ${dayjs(attempt.updated_at).format("YYYY-MM-DD HH:mm:ss")}`}
                      />
                    </List.Item>
                  )}
                />
              )}

              {latestRunningAttempt ? (
                <Card title="完成当前尝试" style={{ marginTop: 24 }}>
                  <Form layout="vertical" onFinish={completeAttempt}>
                    <Form.Item
                      name="status"
                      label="结果状态"
                      initialValue="succeeded"
                      rules={[{ required: true, message: "请选择状态" }]}
                    >
                      <Radio.Group>
                        <Radio.Button value="succeeded">成功</Radio.Button>
                        <Radio.Button value="failed">失败</Radio.Button>
                      </Radio.Group>
                    </Form.Item>
                    <Form.Item name="diff" label="Diff 内容">
                      <Input.TextArea rows={6} placeholder="可选，支持粘贴 git diff" />
                    </Form.Item>
                    <Form.Item name="log" label="执行日志">
                      <Input.TextArea rows={6} placeholder="可选，输入执行日志" />
                    </Form.Item>
                    <Button type="primary" htmlType="submit" loading={completing}>
                      提交结果
                    </Button>
                  </Form>
                </Card>
              ) : null}
            </>
          ) : (
            <Result status="404" title="未找到任务" subTitle="请返回列表重试" />
          )}
        </Card>
        <ArtifactModal
          open={modalVisible}
          title={artifactTitle}
          type={artifactType}
          content={artifactContent}
          loading={artifactLoading}
          error={artifactError}
          onClose={closeArtifactModal}
        />
      </AppLayout>
    </AuthGuard>
  );
}
