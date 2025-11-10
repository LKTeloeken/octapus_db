import type { TreeNode } from "@/shared/models/database.types";

export interface UserServersProps {
  addChildren: (childrens: TreeNode[]) => void;
}
