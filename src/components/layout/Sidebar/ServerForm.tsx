import React, { useState, useEffect } from "react";
import { IPostgreServer, IPostgreServerPrimitive } from "@/models/postgreDb";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";

interface ServerFormProps {
  server?: IPostgreServer;
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (data: IPostgreServerPrimitive) => Promise<void>;
}

export function ServerForm({
  server,
  isOpen,
  onClose,
  onSubmit,
}: ServerFormProps) {
  const [formData, setFormData] = useState<IPostgreServerPrimitive>({
    name: "",
    host: "localhost",
    port: 5432,
    username: "postgres",
    password: "",
  });

  const [errors, setErrors] = useState<Record<string, string>>({});
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Initialize form with server data if editing
  useEffect(() => {
    if (isOpen && server) {
      setFormData({
        name: server.name.toString(),
        host: server.host.toString(),
        port: server.port,
        username: server.username.toString(),
        password: server.password.toString(),
      });
    } else if (isOpen) {
      // Reset form when opening for a new server
      setFormData({
        name: "",
        host: "localhost",
        port: 5432,
        username: "postgres",
        password: "",
      });
    }

    // Clear errors when opening/closing the form
    setErrors({});
  }, [isOpen, server]);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const { name, value, type } = e.target;

    // Convert port to number if it's a number input
    const newValue = name === "port" ? parseInt(value) || 0 : value;

    setFormData((prev) => ({
      ...prev,
      [name]: newValue,
    }));

    // Clear the error for this field when user types
    if (errors[name]) {
      setErrors((prev) => ({
        ...prev,
        [name]: "",
      }));
    }
  };

  const validateForm = (): boolean => {
    const newErrors: Record<string, string> = {};

    if (!formData.name) {
      newErrors.name = "Name is required";
    }

    if (!formData.host) {
      newErrors.host = "Host is required";
    }

    if (!formData.port) {
      newErrors.port = "Port is required";
    } else if (formData.port < 1 || formData.port > 65535) {
      newErrors.port = "Port must be between 1 and 65535";
    }

    if (!formData.username) {
      newErrors.username = "Username is required";
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!validateForm()) {
      return;
    }

    setIsSubmitting(true);
    try {
      await onSubmit(formData);
      onClose();
    } catch (error) {
      console.error("Error submitting form:", error);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {server ? "Edit Server" : "Add New PostgreSQL Server"}
          </DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="grid gap-4">
            <div className="grid gap-2">
              <Label htmlFor="name">Name</Label>
              <Input
                id="name"
                name="name"
                placeholder="My PostgreSQL Server"
                value={formData.name.toString()}
                onChange={handleChange}
                required
              />
              {errors.name && (
                <p className="text-xs text-destructive">{errors.name}</p>
              )}
            </div>

            <div className="grid gap-2">
              <Label htmlFor="host">Host</Label>
              <Input
                id="host"
                name="host"
                placeholder="localhost"
                value={formData.host.toString()}
                onChange={handleChange}
                required
              />
              {errors.host && (
                <p className="text-xs text-destructive">{errors.host}</p>
              )}
            </div>

            <div className="grid gap-2">
              <Label htmlFor="port">Port</Label>
              <Input
                id="port"
                name="port"
                type="number"
                placeholder="5432"
                value={formData.port}
                onChange={handleChange}
                min={1}
                max={65535}
                required
              />
              {errors.port && (
                <p className="text-xs text-destructive">{errors.port}</p>
              )}
            </div>

            <div className="grid gap-2">
              <Label htmlFor="username">Username</Label>
              <Input
                id="username"
                name="username"
                placeholder="postgres"
                value={formData.username.toString()}
                onChange={handleChange}
                required
              />
              {errors.username && (
                <p className="text-xs text-destructive">{errors.username}</p>
              )}
            </div>

            <div className="grid gap-2">
              <Label htmlFor="password">Password</Label>
              <Input
                id="password"
                name="password"
                type="password"
                placeholder="Password"
                value={formData.password?.toString() || ""}
                onChange={handleChange}
              />
              {errors.password && (
                <p className="text-xs text-destructive">{errors.password}</p>
              )}
            </div>
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={onClose}
              disabled={isSubmitting}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={isSubmitting}>
              {isSubmitting
                ? "Saving..."
                : server
                ? "Save Changes"
                : "Add Server"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
