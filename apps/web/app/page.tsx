export default function Page() {
  return (
    <div className="container">
      <header className="header">
        <h1 className="title">Welcome to Surf</h1>
        <p className="subtitle">Nx monorepo with Next.js and Rust</p>
      </header>

      <main className="main">
        <section className="card">
          <h2>Shared Proto Types</h2>
          <p>This project uses Protocol Buffers for type sharing between TypeScript and Rust.</p>
          <pre className="code-block">{`message User {
  string id = 1;
  string name = 2;
  string email = 3;
}`}</pre>
        </section>

        <section className="card">
          <h2>Technology Stack</h2>
          <ul className="tech-list">
            <li><strong>Frontend:</strong> Next.js 15</li>
            <li><strong>Styling:</strong> CSS</li>
            <li><strong>Backend:</strong> Rust CLI</li>
            <li><strong>Types:</strong> Protocol Buffers</li>
            <li><strong>Build:</strong> Nx 20</li>
          </ul>
        </section>

        <section className="card">
          <h2>Project Structure</h2>
          <pre className="code-block">{`surf/
|- apps/web/                # Next.js app
|- crates/cli/              # Rust CLI
|- crates/shared-proto/     # Rust proto types
|- libs/shared/             # TS proto types
\- proto/                   # Proto definitions`}</pre>
        </section>
      </main>

      <footer className="footer">
        <p>Built with Nx, Next.js, and Rust</p>
      </footer>
    </div>
  );
}
