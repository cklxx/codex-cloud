import { useMemo } from "react";
import { Modal, Typography, Empty, Alert, Spin } from "antd";
import { Diff, Hunk, parseDiff } from "react-diff-view";
import { Virtuoso } from "react-virtuoso";

export type ArtifactViewerType = "diff" | "log" | "text";

interface ArtifactModalProps {
  open: boolean;
  title: string;
  type: ArtifactViewerType | null;
  content: string | null;
  loading?: boolean;
  error?: string | null;
  onClose: () => void;
}

const getLogLines = (content: string | null) => {
  if (!content) {
    return [] as string[];
  }
  return content.replace(/\r\n/g, "\n").split("\n");
};

type DiffFileEntry = ReturnType<typeof parseDiff>[number];

function renderDiff(files: DiffFileEntry[]) {
  if (files.length === 0) {
    return <Empty description="暂无 diff 内容" />;
  }

  return files.map((file, index) => {
    const key = file.newPath || file.oldPath || `${file.type}-${index}`;
    return (
      <div key={key} className="artifact-diff-wrapper">
        <Typography.Text className="artifact-diff-title" strong>
          {file.oldPath && file.newPath && file.oldPath !== file.newPath
            ? `${file.oldPath} → ${file.newPath}`
            : file.newPath || file.oldPath || "文件"}
        </Typography.Text>
        <Diff viewType="unified" diffType={file.type} hunks={file.hunks}>
          {(hunks) => hunks.map((hunk) => <Hunk key={hunk.content} hunk={hunk} />)}
        </Diff>
      </div>
    );
  });
}

export function ArtifactModal({
  open,
  title,
  type,
  content,
  loading = false,
  error = null,
  onClose
}: ArtifactModalProps) {
  const { diffFiles, parseError } = useMemo(() => {
    if (type !== "diff" || !content) {
      return { diffFiles: [] as DiffFileEntry[], parseError: null as string | null };
    }

    try {
      return { diffFiles: parseDiff(content), parseError: null };
    } catch (err) {
      console.error("Failed to parse diff artifact", err);
      return { diffFiles: [] as DiffFileEntry[], parseError: "无法解析 diff 内容" };
    }
  }, [content, type]);

  const logLines = useMemo(() => {
    if (type !== "log") {
      return [] as string[];
    }
    return getLogLines(content);
  }, [content, type]);

  const body = useMemo(() => {
    if (loading) {
      return (
        <div className="artifact-modal-loading">
          <Spin size="large" />
        </div>
      );
    }

    if (error) {
      return <Alert type="error" message={error} showIcon />;
    }

    if (type === "diff") {
      if (parseError) {
        return <Alert type="error" message={parseError} showIcon />;
      }
      return <div className="artifact-modal-body">{renderDiff(diffFiles)}</div>;
    }

    if (type === "log") {
      if (logLines.length === 0) {
        return <Empty description="暂无日志" />;
      }
      const height = Math.min(Math.max(logLines.length * 20, 240), 480);
      return (
        <div className="artifact-modal-body">
          <Virtuoso
            style={{ height }}
            data={logLines}
            itemContent={(index, line) => (
              <div className="artifact-log-line">
                <Typography.Text className="artifact-log-line-number" code>
                  {index + 1}
                </Typography.Text>
                <Typography.Text className="artifact-log-line-content">
                  {line.length > 0 ? line : "\u00a0"}
                </Typography.Text>
              </div>
            )}
          />
        </div>
      );
    }

    if (type === "text") {
      return (
        <pre className="artifact-modal-body artifact-text-body">{content ?? ""}</pre>
      );
    }

    return <Empty description="暂无内容" />;
  }, [loading, error, type, parseError, diffFiles, logLines, content]);

  return (
    <Modal
      open={open}
      onCancel={onClose}
      footer={null}
      width={960}
      title={title}
      destroyOnClose
    >
      {body}
    </Modal>
  );
}

export default ArtifactModal;
