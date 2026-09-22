import Link from "next/link";

import { AuthForm } from "@/components/auth-form";
import { GoogleSignIn } from "@/components/google-sign-in";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

export default function SignupPage() {
  return (
    <Card className="auth-card w-full max-w-sm">
      <CardHeader>
        <CardTitle>Create your workspace</CardTitle>
        <CardDescription>Start running campaigns from your own mailbox.</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <GoogleSignIn />
        <AuthForm
          endpoint="/auth/signup"
          submitLabel="Create workspace"
          fields={[
            { name: "org_name", label: "Organization", autoComplete: "organization" },
            { name: "name", label: "Your name", autoComplete: "name" },
            { name: "email", label: "Email", type: "email", autoComplete: "email" },
            {
              name: "password",
              label: "Password",
              type: "password",
              autoComplete: "new-password",
            },
          ]}
        />
        <p className="text-sm text-muted-foreground">
          Already have an account?{" "}
          <Link href="/login" className="underline underline-offset-4">
            Sign in
          </Link>
        </p>
      </CardContent>
    </Card>
  );
}
