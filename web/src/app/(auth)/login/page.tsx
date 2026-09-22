import Link from "next/link";

import { AuthForm } from "@/components/auth-form";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

export default function LoginPage() {
  return (
    <Card className="auth-card w-full max-w-sm">
      <CardHeader>
        <CardTitle>Sign in</CardTitle>
        <CardDescription>Welcome back.</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <AuthForm
          endpoint="/auth/login"
          submitLabel="Sign in"
          fields={[
            { name: "email", label: "Email", type: "email", autoComplete: "email" },
            {
              name: "password",
              label: "Password",
              type: "password",
              autoComplete: "current-password",
            },
          ]}
        />
        <p className="text-sm text-muted-foreground">
          No account?{" "}
          <Link href="/signup" className="underline underline-offset-4">
            Create one
          </Link>
        </p>
      </CardContent>
    </Card>
  );
}
