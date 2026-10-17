// Builds a signed release and the `latest.json` consumed by the auto-updater.
//
//   node scripts/release.mjs                 build only (updater disabled unless the URL is set)
//   node scripts/release.mjs --publish-vps   also upload installer, signature and latest.json to the VPS
//   node scripts/release.mjs --publish       also create the GitHub release with `gh`
//
// Environment:
//   COMMITO_UPDATE_URL          URL of latest.json that installed apps will poll, e.g.
//                                  https://github.com/<owner>/<repo>/releases/latest/download/latest.json
//   COMMITO_DONATION_API_URL    (optional) donation API, e.g. https://doacao.seudominio.com/pix
//   TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)  optional; defaults to ~/.tauri/commito-updater.key(.password)
//
// The private key must never be committed. Losing it means installed apps cannot be updated anymore.

import { execFileSync, spawnSync } from 'node:child_process'
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = resolve(fileURLToPath(new URL('..', import.meta.url)))
const conf = JSON.parse(readFileSync(join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'))
const version = conf.version
const publish = process.argv.includes('--publish')
const publishVps = process.argv.includes('--publish-vps')

// Official distribution server (override with the env vars below).
const SITE = 'https://commito.cauadev.com'
const VPS = process.env.COMMITO_VPS ?? 'root@187.77.246.113'
const VPS_DIR = '/var/www/commito/releases'

const env = { ...process.env }
env.COMMITO_UPDATE_URL ??= `${SITE}/releases/latest.json`
env.COMMITO_DONATION_API_URL ??= `${SITE}/api/pix`
env.COMMITO_RELEASE_BASE_URL ??= `${SITE}/releases`
const keyFile = join(homedir(), '.tauri', 'commito-updater.key')
if (!env.TAURI_SIGNING_PRIVATE_KEY) {
  if (!existsSync(keyFile)) throw new Error(`Chave de assinatura não encontrada: ${keyFile}`)
  env.TAURI_SIGNING_PRIVATE_KEY = readFileSync(keyFile, 'utf8')
  const pw = keyFile.replace(/\.key$/, '.password')
  if (existsSync(pw)) env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD = readFileSync(pw, 'utf8').trim()
}
const updateUrl = env.COMMITO_UPDATE_URL
if (!updateUrl) console.warn('⚠ COMMITO_UPDATE_URL não definida: esta build NÃO procurará atualizações.')
if (!env.COMMITO_DONATION_API_URL) console.warn('⚠ COMMITO_DONATION_API_URL não definida: o botão de doação mostrará "não configurado".')

console.log(`→ Build Commito v${version}`)
const r = spawnSync('pnpm tauri build', { cwd: root, env, stdio: 'inherit', shell: true })
if (r.status !== 0) process.exit(r.status ?? 1)

const bundleDir = join(root, 'src-tauri', 'target', 'release', 'bundle', 'nsis')
const setup = `Commito_${version}_x64-setup.exe`
const sigFile = join(bundleDir, `${setup}.sig`)
if (!existsSync(sigFile)) throw new Error(`Assinatura não gerada: ${sigFile}`)

const { changelog } = await import(pathToFileURL(join(root, 'src', 'changelog.ts')).href)
const entry = changelog.find((c) => c.version === version)
const labels = { new: 'Novo', improved: 'Melhoria', fixed: 'Correção' }
const notes = entry ? entry.items.map((i) => `• ${labels[i.type]}: ${i.text}`).join('\n') : `Commito ${version}`

// GitHub release asset URL derived from the latest.json URL (…/releases/latest/download/latest.json).
const gh = updateUrl?.match(/^https:\/\/github\.com\/([^/]+)\/([^/]+)\/releases\//)
const assetBase = env.COMMITO_RELEASE_BASE_URL ?? (gh ? `https://github.com/${gh[1]}/${gh[2]}/releases/download/v${version}` : null)
const latest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': {
      signature: readFileSync(sigFile, 'utf8').trim(),
      url: assetBase ? `${assetBase}/${setup}` : `<defina COMMITO_RELEASE_BASE_URL>/${setup}`,
    },
  },
}
const latestFile = join(bundleDir, 'latest.json')
writeFileSync(latestFile, JSON.stringify(latest, null, 2))
console.log(`✓ ${join(bundleDir, setup)}\n✓ ${sigFile}\n✓ ${latestFile}`)

if (publishVps) {
  // Versioned files first; latest.json last (via rename) so clients never see a feed
  // that points to a file that is not there yet.
  execFileSync('scp', ['-q', join(bundleDir, setup), sigFile, `${VPS}:${VPS_DIR}/`], { stdio: 'inherit' })
  execFileSync('scp', ['-q', latestFile, `${VPS}:${VPS_DIR}/latest.json.tmp`], { stdio: 'inherit' })
  execFileSync('ssh', [VPS, `chmod 644 ${VPS_DIR}/* && mv ${VPS_DIR}/latest.json.tmp ${VPS_DIR}/latest.json && ln -sfn ${setup} ${VPS_DIR}/Commito-setup.exe`], { stdio: 'inherit' })
  console.log(`✓ Publicado em ${SITE}/releases/${setup}`)
}

if (publish) {
  if (!gh) throw new Error('--publish requer COMMITO_UPDATE_URL apontando para um repositório GitHub.')
  execFileSync('gh', ['release', 'create', `v${version}`, join(bundleDir, setup), sigFile, latestFile, '--repo', `${gh[1]}/${gh[2]}`, '--title', `Commito v${version}`, '--notes', notes], { stdio: 'inherit' })
  console.log('✓ Release publicada.')
}
