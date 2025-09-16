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
  actions?: React.ReactNode; // custom action elements (e.g., popover trigger)
  isExpanded?: boolean;
  hasChildren?: boolean;
  disabled?: boolean;
}

const BaseCell: React.FC<BaseCellProps> = ({
  icon,
  primaryText,
  secondaryText,
  onClick,
  secondaryAction,
  actions,
  isExpanded,
  hasChildren = true,
  disabled = false,
}) => {
  const showChevron = true;

  return (
    <>
      <ListItem
        disablePadding
        secondaryAction={
          <div className="flex items-center space-x-1">
            {actions && <div className="flex items-center">{actions}</div>}
            {secondaryAction ||
              (hasChildren && (
                <div className="ml-auto pr-2">
                  {showChevron &&
                    (isExpanded ? (
                      <ChevronDown className="size-4" />
                    ) : (
                      <ChevronRight className="size-4" />
                    ))}
                </div>
              ))}
          </div>
        }
      >
        <ListItemButton onClick={onClick} className="py-1" disabled={disabled}>
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
