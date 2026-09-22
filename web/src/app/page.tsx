import type { Metadata } from "next";
import Link from "next/link";
import { ArrowDown, ArrowRight, Check, Clock3, GitBranch, Inbox, Mail, Send, Users } from "lucide-react";

import { buttonVariants } from "@/components/ui/button";
import { getCurrentUser } from "@/lib/server-api";
import styles from "./home.module.css";

export const metadata: Metadata = {
  title: "Cold Email — Turn outreach into conversations",
  description: "Run thoughtful email campaigns from your own mailbox. Organize leads, personalize sequences, and keep track of replies in one workspace.",
};

const steps = [
  { number: "01", icon: Mail, title: "Bring your mailbox.", description: "Connect Gmail and send from an address that’s already yours. Keep your identity at the center of every conversation." },
  { number: "02", icon: Users, title: "Find your people.", description: "Import contacts from a CSV and organize them into lead lists. Bring names, companies, and custom fields along." },
  { number: "03", icon: Send, title: "Make the introduction.", description: "Write a personalized sequence, set your follow-ups, and start your campaign. Track replies as they come in." },
];

export default async function Home() {
  const user = await getCurrentUser();

  return (
    <div className={styles.home}>
      <a href="#main-content" className="skip-link">Skip to content</a>
      <header className={styles.header}>
        <div className={styles.headerInner}>
          <Link href="/" aria-label="Cold Email home" className={styles.brand}><Mail size={24} strokeWidth={1.5} aria-hidden="true" />Cold Email</Link>
          <nav aria-label="Main navigation" className={styles.nav}>
            <a href="#product">Product</a><a href="#how-it-works">How it works</a>
          </nav>
          <div className={styles.headerActions}>
            {user ? (
              <Link href="/dashboard" className={buttonVariants()}>Go to dashboard <ArrowRight aria-hidden="true" /></Link>
            ) : (
              <>
                <Link href="/login" className={styles.signIn}>Sign in</Link>
                <Link href="/signup" className={buttonVariants()}>Get started <ArrowRight aria-hidden="true" /></Link>
              </>
            )}
          </div>
        </div>
      </header>

      <main id="main-content" className={styles.main}>
        <section className={styles.hero} aria-labelledby="hero-title">
          <div className={styles.heroContent}>
            <p className="eyebrow">Personal outreach. Thoughtfully organized.</p>
            <h1 id="hero-title">Good conversations<br />start with an email.</h1>
            <p className={styles.heroDescription}>Your mailbox. Your message. One focused workspace to turn<br className={styles.desktopBreak} /> a list of contacts into your next conversation.</p>
            <div className={styles.heroActions}>
              <Link href="/signup" className={buttonVariants({ className: styles.primaryCta })}>Create your workspace <ArrowRight aria-hidden="true" /></Link>
              <a href="#product" className={buttonVariants({ variant: "outline", className: styles.secondaryCta })}>Explore the workflow <ArrowDown aria-hidden="true" /></a>
            </div>
            <p className={styles.heroNote}>Built around your Gmail inbox.</p>
          </div>
          <div className={styles.heroCaption}><span>FROM FIRST HELLO TO FOLLOW-UP</span><span>01 — THE WORKSPACE</span></div>
        </section>

        <section id="product" className={styles.product} aria-label="Example campaign preview">
          <div className={styles.previewHeader}>
            <div className={styles.previewBrand}><Mail size={18} strokeWidth={1.5} aria-hidden="true" /><span>Cold Email</span><span className={styles.slash}>/</span><span>Campaigns</span></div>
            <span className="eyebrow">Example workspace</span>
          </div>
          <div className={styles.previewTitle}><div><p className="eyebrow">Campaign / Introduction</p><h2>A thoughtful first hello.</h2></div><span className={styles.draft}>Draft</span></div>
          <div className={styles.previewBody}>
            <div className={styles.sequence}>
              <p className="eyebrow">Your sequence</p>
              <div className={styles.sequenceStep}><span className={styles.stepIcon}><Send size={16} aria-hidden="true" /></span><div><strong>The introduction</strong><span>Day 1 · Start a conversation</span></div><span className={styles.stepNumber}>01</span></div>
              <div className={styles.delay}><Clock3 size={12} aria-hidden="true" /> Wait 3 days</div>
              <div className={styles.followupStep}><span className={styles.stepIcon}><GitBranch size={16} aria-hidden="true" /></span><div><strong>A friendly follow-up</strong><span>In the same email thread</span></div><span className={styles.stepNumber}>02</span></div>
              <div className={styles.sequenceNote}><Check size={14} aria-hidden="true" /><span>Personalized with your lead’s details</span></div>
            </div>
            <div className={styles.message}>
              <div className={styles.messageMeta}><span>To</span><span>Morgan <span className={styles.sampleEmail}>&lt;morgan@example.com&gt;</span></span></div>
              <div className={styles.messageMeta}><span>Subject</span><span>A thought for your team</span></div>
              <div className={styles.messageBody}><p>Hi <span className={styles.mergeTag}>Morgan</span>,</p><p>I came across your work at <span className={styles.mergeTag}>Fieldwork</span> and had an idea I thought might be useful for your team.</p><p>Would you be open to a quick conversation next week?</p><p>Best,<br />Alex</p></div>
              <div className={styles.messageFooter}><span className="eyebrow">Message preview</span><span>Simple. Personal. Yours.</span></div>
            </div>
          </div>
        </section>

        <div className={styles.capabilities} aria-label="Core capabilities"><span><Mail aria-hidden="true" /> Gmail connection</span><span><Users aria-hidden="true" /> CSV lead imports</span><span><GitBranch aria-hidden="true" /> Email sequences</span><span><Inbox aria-hidden="true" /> Reply tracking</span></div>

        <section id="how-it-works" className={styles.workflow} aria-labelledby="workflow-title">
          <div className={styles.sectionIntro}><div><p className="eyebrow">A clear path to your first send</p><h2 id="workflow-title">Less setup.<br />More introductions.</h2></div><p>Everything you need to put a thoughtful campaign together, in the order you need it.</p></div>
          <ol className={styles.steps}>{steps.map(({ number, icon: Icon, title, description }) => <li key={number}><div className={styles.stepHeading}><Icon size={22} strokeWidth={1.5} aria-hidden="true" /><span className="eyebrow">{number}</span></div><h3>{title}</h3><p>{description}</p></li>)}</ol>
        </section>

        <section className={styles.details} aria-label="Campaign features">
          <article><div className={styles.detailHeading}><GitBranch size={20} strokeWidth={1.5} aria-hidden="true" /><p className="eyebrow">Make it personal</p></div><h2>A sequence.<br />With a human touch.</h2><p>Use names, companies, and custom fields to write messages that fit each contact. Add fallbacks so missing details never leave a blank in your email.</p><div className={styles.codeExample}><span className="eyebrow">A small detail. A better introduction.</span><code>{"Hi {{first_name|there}},"}</code><span className={styles.codeResult}><ArrowRight size={14} aria-hidden="true" /> Hi Morgan,</span></div></article>
          <article><div className={styles.detailHeading}><Inbox size={20} strokeWidth={1.5} aria-hidden="true" /><p className="eyebrow">Stay in the loop</p></div><h2>Know what’s moving.<br />See what needs you.</h2><p>Keep active campaigns, daily sending capacity, and recent replies in view. Spot connection issues and messages that need attention from your dashboard.</p><ul className={styles.checklist}><li><Check aria-hidden="true" /> Per-mailbox daily sending limits</li><li><Check aria-hidden="true" /> Replies collected in your overview</li><li><Check aria-hidden="true" /> One shared workspace for your team</li></ul></article>
        </section>

        <section className={styles.closing} aria-labelledby="closing-title"><Mail size={32} strokeWidth={1} aria-hidden="true" /><p className="eyebrow">Your next conversation is out there</p><h2 id="closing-title">Start with a hello.</h2><p>Bring your mailbox. We’ll help you organize the rest.</p><Link href="/signup" className={buttonVariants({ className: styles.primaryCta })}>Create your workspace <ArrowRight aria-hidden="true" /></Link></section>
      </main>

      <footer className={styles.footer}><Link href="/" className={styles.brand}><Mail size={20} strokeWidth={1.5} aria-hidden="true" />Cold Email</Link><span className="eyebrow">Your mailbox. Your conversations.</span><nav aria-label="Footer navigation"><Link href={user ? "/dashboard" : "/login"}>{user ? "Go to dashboard" : "Sign in"}</Link>{!user && <Link href="/dashboard">Workspace <span aria-hidden="true">↗</span></Link>}</nav></footer>
    </div>
  );
}
