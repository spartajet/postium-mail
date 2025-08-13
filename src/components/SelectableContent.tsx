import React from "react";
import { makeStyles, mergeClasses } from "@fluentui/react-components";

const useStyles = makeStyles({
  root: {
    userSelect: "text",
    "-webkit-user-select": "text",
    "-moz-user-select": "text",
    "-ms-user-select": "text",
    cursor: "text",
  },
});

interface SelectableContentProps {
  children: React.ReactNode;
  className?: string;
  as?: React.ElementType;
}

export const SelectableContent: React.FC<SelectableContentProps> = ({
  children,
  className,
  as: Component = "div",
}) => {
  const styles = useStyles();

  return (
    <Component className={mergeClasses(styles.root, "selectable", className)}>
      {children}
    </Component>
  );
};
