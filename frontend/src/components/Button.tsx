import type { ButtonHTMLAttributes } from "react";
import { cn } from "../lib/cn";

type ButtonVariant = "primary" | "secondary" | "outline" | "ghost" | "destructive" | "link";
type ButtonSize = "default" | "sm" | "xs";

type ButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "id"> & {
  id: string;
  variant?: ButtonVariant;
  size?: ButtonSize;
};

export function Button({
  id,
  className,
  variant = "secondary",
  size = "default",
  type = "button",
  ...props
}: ButtonProps) {
  return (
    <button
      id={id}
      type={type}
      className={cn(
        "inline-flex items-center justify-center gap-1.5 rounded-md font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-40",
        size === "default" && "h-8 px-2.5 text-sm",
        size === "sm" && "h-7 px-2 text-xs",
        size === "xs" && "h-6 gap-1 px-1.5 text-xs",
        variant === "primary" && "bg-sky-600 text-white hover:bg-sky-500",
        variant === "secondary" && "border border-zinc-700 bg-zinc-800 text-zinc-100 hover:bg-zinc-700",
        variant === "outline" && "border border-zinc-600 bg-transparent text-zinc-100 hover:bg-zinc-800",
        variant === "ghost" && "text-zinc-300 hover:bg-zinc-800",
        variant === "destructive" && "bg-red-600 text-white hover:bg-red-500",
        variant === "link" && "h-auto px-0 text-sky-400 underline-offset-4 hover:text-sky-300 hover:underline",
        className,
      )}
      {...props}
    />
  );
}
