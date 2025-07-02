import React from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import {
  ListItem,
  ListItemText,
  ListItemButton,
  ListItemIcon,
} from "@/components/ui/list-item";

interface BaseCellProps {
  icon: React.ReactNode;
  primaryText: string;
  secondaryText?: string;
  onClick?: () => void;
  secondaryAction?: React.ReactNode;
  isExpanded?: boolean;
  hasChildren?: boolean;
}

const BaseCell: React.FC<BaseCellProps> = ({
  icon,
  primaryText,
  secondaryText,
  onClick,
  secondaryAction,
  isExpanded,
  hasChildren = false,
}) => {
  const showChevron = true;

  return (
    <>
      <ListItem
        disablePadding
        secondaryAction={
          secondaryAction || (
            <div className="ml-auto pr-2">
              {showChevron &&
                (isExpanded ? (
                  <ChevronDown className="size-4" />
                ) : (
                  <ChevronRight className="size-4" />
                ))}
            </div>
          )
        }
      >
        <ListItemButton onClick={onClick} className="py-1">
          <ListItemIcon className="flex items-center justify-center">
            {icon}
          </ListItemIcon>
          <ListItemText primary={primaryText} secondary={secondaryText} />
        </ListItemButton>
      </ListItem>
    </>
  );
};

export default BaseCell;
