<script lang="ts">
  // #199 — About, adapted from the macOS About pane in the prototype's
  // settings-about reference to the Windows/Linux port.
  const VERSION = '0.1.0';
</script>

<h2>About</h2>
<div class="about">
  <div class="mark"><span><i></i><i></i></span></div>
  <div class="name">Morrow</div>
  <div class="version">Version {VERSION}</div>
  <p>
    Real-time AI coding agent status panel for Windows and Linux<br />
    Talks to agents over a Unix socket or a named pipe
  </p>
  <div class="links">
    <a href="https://github.com/amaldevice/CodeIsland" target="_blank" rel="noreferrer">GitHub</a>
    <a href="https://github.com/amaldevice/CodeIsland/issues" target="_blank" rel="noreferrer">Issues</a>
    <a href="https://github.com/amaldevice/CodeIsland/releases" target="_blank" rel="noreferrer">What's New</a>
  </div>
  <button class="b" disabled title="Updates are handled by the package manager on this platform">
    Check for Updates…
  </button>
</div>
