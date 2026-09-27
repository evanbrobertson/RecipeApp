/**
 * The emails Crumb sends (verification, password reset, household invites), through Amazon
 * SES when it's configured. Without it nothing is sent: the link is logged instead, so a
 * test deploy still works, and sign-up doesn't wait on a verified email.
 */
import { AwsClient } from "aws4fetch"

export type Mail = { to: string; subject: string; text: string }

export interface Mailer {
  /** Whether mail actually leaves (verification is only required when it does). */
  readonly enabled: boolean
  send(mail: Mail): Promise<void>
}

export type MailerEnv = Record<string, string | undefined>

/** SES v2 when `SES_REGION`, `EMAIL_FROM` and AWS keys are set; otherwise the log. */
export function mailerFromEnv(env: MailerEnv = process.env): Mailer {
  const region = env.SES_REGION ?? env.AWS_REGION
  const from = env.EMAIL_FROM
  const accessKeyId = env.AWS_ACCESS_KEY_ID
  const secretAccessKey = env.AWS_SECRET_ACCESS_KEY
  if (region && from && accessKeyId && secretAccessKey) {
    return sesMailer({ region, from, accessKeyId, secretAccessKey })
  }
  return logMailer()
}

export function logMailer(): Mailer & { sent: Mail[] } {
  const sent: Mail[] = []
  return {
    enabled: false,
    sent,
    async send(mail) {
      sent.push(mail)
      // The address stays out of the log; the link is what an operator needs
      console.info(`[mail] not sent (no email provider): ${mail.subject}\n${mail.text}`)
    },
  }
}

function sesMailer(opts: {
  region: string
  from: string
  accessKeyId: string
  secretAccessKey: string
}): Mailer {
  const aws = new AwsClient({
    accessKeyId: opts.accessKeyId,
    secretAccessKey: opts.secretAccessKey,
    region: opts.region,
    service: "ses",
  })
  const url = `https://email.${opts.region}.amazonaws.com/v2/email/outbound-emails`
  return {
    enabled: true,
    async send(mail) {
      const res = await aws.fetch(url, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          FromEmailAddress: opts.from,
          Destination: { ToAddresses: [mail.to] },
          Content: {
            Simple: {
              Subject: { Data: mail.subject, Charset: "UTF-8" },
              Body: { Text: { Data: mail.text, Charset: "UTF-8" } },
            },
          },
        }),
      })
      if (!res.ok) throw new Error(`SES answered ${res.status}: ${await res.text()}`)
    },
  }
}
