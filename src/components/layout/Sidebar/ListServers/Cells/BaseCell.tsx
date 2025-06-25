import React, { useState } from "react";
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
  onClick?: () => void;
  secondaryAction?: React.ReactNode;
  children?: React.ReactNode;
}

const BaseCell: React.FC<BaseCellProps> = ({
  icon,
  primaryText,
  onClick,
  secondaryAction,
  children,
}) => {
  const [isExpanded, setIsExpanded] = useState(false);

  const handleClick = () => {
    setIsExpanded(!isExpanded);
    onClick?.();
  };

  return (
    <>
      <ListItem
        disablePadding
        secondaryAction={
          secondaryAction || (
            <div className="ml-auto pr-2">
              {isExpanded ? (
                <ChevronDown className="size-4" />
              ) : (
                <ChevronRight className="size-4" />
              )}
            </div>
          )
        }
      >
        <ListItemButton onClick={handleClick} className="py-1">
          <ListItemIcon className="flex items-center justify-center">
            {icon}
          </ListItemIcon>
          <ListItemText primary={primaryText} />
        </ListItemButton>
      </ListItem>

      {isExpanded && <div className="pl-4">{children}</div>}
    </>
  );
};

export default BaseCell;
