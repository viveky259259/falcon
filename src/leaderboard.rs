use crate::ai_score::score::AiCodeScore;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardInput {
    pub generated_at: String,
    pub packages: Vec<LeaderboardPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardPackage {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub pub_points: u32,
    #[serde(default)]
    pub likes: Option<u32>,
    #[serde(default)]
    pub popularity: Option<f64>,
    #[serde(default)]
    pub pub_url: Option<String>,
    pub score: AiCodeScore,
}

pub fn build_from_file(input: &Path, output_dir: &Path) -> anyhow::Result<()> {
    let json = fs::read_to_string(input)
        .with_context(|| format!("failed to read leaderboard input {}", input.display()))?;
    let data: LeaderboardInput = serde_json::from_str(&json)
        .with_context(|| format!("invalid leaderboard input {}", input.display()))?;
    build_site(&data, output_dir)
}

pub fn build_site(data: &LeaderboardInput, output_dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    fs::create_dir_all(output_dir.join("packages"))
        .with_context(|| format!("failed to create {}", output_dir.join("packages").display()))?;

    let mut packages = data.packages.clone();
    packages.sort_by_key(|pkg| {
        (
            Reverse(pkg.score.overall),
            Reverse(pkg.pub_points),
            pkg.name.to_ascii_lowercase(),
        )
    });

    fs::write(output_dir.join("index.html"), render_index(data, &packages))?;
    for package in &packages {
        fs::write(
            package_page_path(output_dir, package),
            render_package(package),
        )?;
    }

    Ok(())
}

fn package_page_path(output_dir: &Path, package: &LeaderboardPackage) -> PathBuf {
    output_dir
        .join("packages")
        .join(format!("{}.html", slug(&package.name)))
}

fn render_index(data: &LeaderboardInput, packages: &[LeaderboardPackage]) -> String {
    let rows = packages
        .iter()
        .enumerate()
        .map(|(index, package)| render_index_row(index + 1, package))
        .collect::<String>();

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Falcon pub.dev Leaderboard</title>
  <style>{css}</style>
</head>
<body>
  <main>
    <header>
      <p class="eyebrow">Generated {generated_at}</p>
      <h1>Falcon pub.dev Leaderboard</h1>
      <p class="lede">Static Falcon score reports for pub.dev packages. Click column headers to sort.</p>
    </header>
    <table id="leaderboard">
      <thead>
        <tr>
          <th data-type="number">#</th>
          <th>Package</th>
          <th data-type="number">Falcon</th>
          <th data-type="number">Pub points</th>
          <th data-type="number">Likes</th>
          <th data-type="number">Popularity</th>
          <th>Grade</th>
          <th data-type="number">Issues</th>
        </tr>
      </thead>
      <tbody>{rows}</tbody>
    </table>
    <section>
      <h2>Methodology</h2>
      <p>Falcon scores checked-out package sources with <code>falcon score --json</code>. The leaderboard is a static snapshot: it does not call pub.dev or any live API from the browser.</p>
      <p>Package metadata and scores come from the generated input manifest used by CI. Rows are sorted by Falcon score, then pub points, then package name.</p>
      <h2>Opt out</h2>
      <p>Package maintainers can opt out by opening an issue in the Falcon repository with the package name and ownership proof. Opted-out packages are removed from the next generated snapshot.</p>
    </section>
  </main>
  <script>{script}</script>
</body>
</html>
"#,
        css = page_css(),
        generated_at = escape_html(&data.generated_at),
        rows = rows,
        script = sort_script(),
    )
}

fn render_index_row(rank: usize, package: &LeaderboardPackage) -> String {
    let page = format!("packages/{}.html", slug(&package.name));
    let likes = package
        .likes
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    let popularity = package
        .popularity
        .map(|value| format!("{value:.1}"))
        .unwrap_or_else(|| "-".to_string());

    format!(
        r#"<tr>
  <td>{rank}</td>
  <td><a href="{page}">{name}</a><div class="muted">{description}</div></td>
  <td>{score}</td>
  <td>{pub_points}</td>
  <td>{likes}</td>
  <td>{popularity}</td>
  <td><span class="grade grade-{grade_class}">{grade}</span></td>
  <td>{issues}</td>
</tr>
"#,
        rank = rank,
        page = page,
        name = escape_html(&package.name),
        description = escape_html(package.description.as_deref().unwrap_or("")),
        score = package.score.overall,
        pub_points = package.pub_points,
        likes = likes,
        popularity = popularity,
        grade = package.score.grade,
        grade_class = package.score.grade.to_string().to_ascii_lowercase(),
        issues = package.score.total_issues,
    )
}

fn render_package(package: &LeaderboardPackage) -> String {
    let pub_url = package
        .pub_url
        .clone()
        .unwrap_or_else(|| format!("https://pub.dev/packages/{}", package.name));
    let version = package.version.as_deref().unwrap_or("unknown");

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{name} - Falcon Score</title>
  <style>{css}</style>
</head>
<body>
  <main>
    <p><a href="../index.html">Back to leaderboard</a></p>
    <h1>{name}</h1>
    <p class="lede">{description}</p>
    <dl class="summary">
      <div><dt>Version</dt><dd>{version}</dd></div>
      <div><dt>Falcon score</dt><dd>{score}/100</dd></div>
      <div><dt>Grade</dt><dd>{grade}</dd></div>
      <div><dt>Total issues</dt><dd>{issues}</dd></div>
      <div><dt>Pub points</dt><dd>{pub_points}</dd></div>
    </dl>
    <p><a href="{pub_url}">Open on pub.dev</a></p>
    <h2>Score Dimensions</h2>
    <table>
      <thead><tr><th>Dimension</th><th>Score</th><th>Findings</th></tr></thead>
      <tbody>
        {dimension_rows}
      </tbody>
    </table>
  </main>
</body>
</html>
"#,
        css = page_css(),
        name = escape_html(&package.name),
        description = escape_html(package.description.as_deref().unwrap_or("")),
        version = escape_html(version),
        score = package.score.overall,
        grade = package.score.grade,
        issues = package.score.total_issues,
        pub_points = package.pub_points,
        pub_url = escape_html(&pub_url),
        dimension_rows = render_dimension_rows(&package.score),
    )
}

fn render_dimension_rows(score: &AiCodeScore) -> String {
    [
        (
            "Resource Safety",
            score.resource_safety.score,
            &score.resource_safety.findings,
        ),
        (
            "Error Handling",
            score.error_handling.score,
            &score.error_handling.findings,
        ),
        (
            "Type Safety",
            score.type_safety.score,
            &score.type_safety.findings,
        ),
        ("Security", score.security.score, &score.security.findings),
        (
            "Convention Match",
            score.convention_match.score,
            &score.convention_match.findings,
        ),
        (
            "Complexity",
            score.complexity.score,
            &score.complexity.findings,
        ),
    ]
    .iter()
    .map(|(name, value, findings)| {
        let findings = if findings.is_empty() {
            "None".to_string()
        } else {
            findings
                .iter()
                .map(|finding| escape_html(finding))
                .collect::<Vec<_>>()
                .join("<br>")
        };
        format!(
            "<tr><td>{}</td><td>{}/100</td><td>{}</td></tr>",
            escape_html(name),
            value,
            findings
        )
    })
    .collect::<String>()
}

fn page_css() -> &'static str {
    r#"
body { margin: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #020617; color: #f8fafc; }
main { max-width: 1120px; margin: 0 auto; padding: 48px 24px; }
a { color: #06b6d4; text-decoration: none; }
a:hover { text-decoration: underline; }
.eyebrow, .muted { color: #94a3b8; }
.lede { color: #cbd5e1; max-width: 720px; }
table { width: 100%; border-collapse: collapse; margin-top: 24px; }
th, td { border-bottom: 1px solid #1e293b; padding: 12px; text-align: left; vertical-align: top; }
th { color: #94a3b8; cursor: pointer; font-size: 12px; text-transform: uppercase; letter-spacing: .05em; }
tr:hover td { background: rgba(6, 182, 212, .05); }
.grade { display: inline-block; min-width: 2rem; text-align: center; border-radius: 4px; padding: 2px 8px; font-weight: 700; }
.grade-a, .grade-b { background: rgba(34, 197, 94, .15); color: #22c55e; }
.grade-c { background: rgba(234, 179, 8, .15); color: #eab308; }
.grade-d, .grade-f { background: rgba(239, 68, 68, .15); color: #ef4444; }
.summary { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 16px; margin: 24px 0; }
.summary div { border: 1px solid #1e293b; padding: 16px; }
dt { color: #94a3b8; font-size: 12px; text-transform: uppercase; }
dd { margin: 4px 0 0; font-size: 22px; font-weight: 700; }
"#
}

fn sort_script() -> &'static str {
    r##"
document.querySelectorAll("th").forEach((header, index) => {
  header.addEventListener("click", () => {
    const tbody = document.querySelector("#leaderboard tbody");
    const rows = Array.from(tbody.querySelectorAll("tr"));
    const numeric = header.dataset.type === "number";
    const direction = header.dataset.direction === "asc" ? -1 : 1;
    header.dataset.direction = direction === 1 ? "asc" : "desc";
    rows.sort((a, b) => {
      const left = a.children[index].innerText.trim();
      const right = b.children[index].innerText.trim();
      if (numeric) {
        return direction * ((parseFloat(right) || 0) - (parseFloat(left) || 0));
      }
      return direction * left.localeCompare(right);
    });
    rows.forEach(row => tbody.appendChild(row));
  });
});
"##
}

fn slug(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_score::score::{DimensionScore, Grade};

    #[test]
    fn builds_index_and_package_pages() {
        let temp = tempfile::tempdir().unwrap();
        let data = LeaderboardInput {
            generated_at: "2026-06-29T00:00:00Z".to_string(),
            packages: vec![package("beta", 80, 160), package("alpha", 95, 120)],
        };

        build_site(&data, temp.path()).unwrap();

        let index = fs::read_to_string(temp.path().join("index.html")).unwrap();
        assert!(index.contains("Falcon pub.dev Leaderboard"));
        assert!(index.contains("Methodology"));
        assert!(index.contains("Opt out"));
        assert!(!index.contains("fetch("));
        assert!(!index.contains("pub.dev/api"));
        assert!(index.find("alpha").unwrap() < index.find("beta").unwrap());
        assert!(index.contains("packages/alpha.html"));

        let package = fs::read_to_string(temp.path().join("packages/alpha.html")).unwrap();
        assert!(package.contains("Score Dimensions"));
        assert!(package.contains("95/100"));
    }

    #[test]
    fn escapes_package_content() {
        let temp = tempfile::tempdir().unwrap();
        let mut package = package("<bad>", 60, 1);
        package.description = Some("<script>alert(1)</script>".to_string());
        let data = LeaderboardInput {
            generated_at: "now".to_string(),
            packages: vec![package],
        };

        build_site(&data, temp.path()).unwrap();
        let index = fs::read_to_string(temp.path().join("index.html")).unwrap();
        assert!(index.contains("&lt;bad&gt;"));
        assert!(!index.contains("<script>alert"));
    }

    fn package(name: &str, overall: u32, pub_points: u32) -> LeaderboardPackage {
        LeaderboardPackage {
            name: name.to_string(),
            version: Some("1.0.0".to_string()),
            description: Some(format!("{name} package")),
            pub_points,
            likes: Some(10),
            popularity: Some(90.0),
            pub_url: None,
            score: AiCodeScore {
                overall,
                resource_safety: dim(100),
                error_handling: dim(90),
                type_safety: dim(80),
                security: dim(100),
                convention_match: dim(70),
                complexity: dim(60),
                file_count: 4,
                total_issues: 2,
                grade: Grade::A,
            },
        }
    }

    fn dim(score: u32) -> DimensionScore {
        DimensionScore {
            score,
            findings: Vec::new(),
        }
    }
}
