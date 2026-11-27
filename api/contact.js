/**
 * Vercel Serverless Function: POST /api/contact
 * Receives project brief submissions from the Rashi Agency website.
 *
 * Optional Vercel Environment Variables:
 * - RESEND_API_KEY: Resend API key to deliver emails directly to your inbox
 * - CONTACT_TO_EMAIL: Destination email address (e.g. you@yourdomain.com)
 * - CONTACT_FROM_EMAIL: Verified sender (defaults to "Rashi Agency <onboarding@resend.dev>")
 * - WEBHOOK_URL: Optional Slack/Discord/Zapier/Make webhook URL to forward submissions
 */
export default async function handler(req, res) {
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

  if (req.method === 'OPTIONS') {
    return res.status(200).end();
  }

  if (req.method !== 'POST') {
    res.setHeader('Allow', 'POST');
    return res.status(405).json({ ok: false, error: 'Method not allowed' });
  }

  try {
    const body = typeof req.body === 'string' ? JSON.parse(req.body) : (req.body || {});
    const name = String(body.name || '').trim();
    const email = String(body.email || '').trim();
    const need = String(body.need || 'Website').trim();
    const budget = String(body.budget || 'Not shared').trim();
    const details = String(body.details || 'Not shared').trim();

    if (!name || !email || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
      return res.status(400).json({ ok: false, error: 'Please provide a valid name and work email.' });
    }

    const payload = {
      name,
      email,
      need,
      budget,
      details,
      submittedAt: new Date().toISOString()
    };

    const deliveries = [];

    // 1. Optional Resend email delivery
    if (process.env.RESEND_API_KEY && process.env.CONTACT_TO_EMAIL) {
      const emailResp = await fetch('https://api.resend.com/emails', {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${process.env.RESEND_API_KEY}`,
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({
          from: process.env.CONTACT_FROM_EMAIL || 'Rashi Agency <onboarding@resend.dev>',
          to: [process.env.CONTACT_TO_EMAIL],
          reply_to: email,
          subject: `New Project Brief: ${need} — ${name}`,
          text: [
            `New Project Brief for Rashi Agency`,
            `----------------------------------`,
            `Name: ${name}`,
            `Work Email: ${email}`,
            `Need: ${need}`,
            `Budget Range: ${budget}`,
            `Submitted: ${payload.submittedAt}`,
            ``,
            `Details:`,
            details
          ].join('\n')
        })
      });

      if (!emailResp.ok) {
        const errText = await emailResp.text();
        console.error('Resend delivery failed:', errText);
        return res.status(502).json({ ok: false, error: 'Email delivery service returned an error.' });
      }
      deliveries.push('email');
    }

    // 2. Optional Webhook forwarding (Slack, Discord, Zapier, Make, etc.)
    if (process.env.WEBHOOK_URL) {
      const hookResp = await fetch(process.env.WEBHOOK_URL, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (hookResp.ok) deliveries.push('webhook');
    }

    console.log('PROJECT_BRIEF_SUBMISSION', JSON.stringify(payload));

    return res.status(200).json({
      ok: true,
      deliveredVia: deliveries.length ? deliveries : ['vercel-logs'],
      message: deliveries.includes('email')
        ? 'Brief sent! We will reply to your work email shortly.'
        : 'Brief received and logged. Configure RESEND_API_KEY & CONTACT_TO_EMAIL in Vercel to route directly to your inbox.'
    });
  } catch (err) {
    console.error('Contact endpoint error:', err);
    return res.status(500).json({ ok: false, error: 'Unable to process submission.' });
  }
}
