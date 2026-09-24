export type Density = "compact" | "comfortable" | "large";

export type FileKind = "doc" | "sheet" | "pdf" | "folder" | "image" | "video";

export interface FileItem {
  id: number;
  name: string;
  path: string;
  kind: FileKind;
  size: string;
  modified: string;
  summary?: string;
}
