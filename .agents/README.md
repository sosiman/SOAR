# .agents - memoria tecnica y skills del proyecto

Este directorio es la memoria duradera del proyecto para agentes. DSH lo descubre automaticamente:

- Skills de proyecto: .agents/skills/<nombre>/SKILL.md (o .agents/skills/<nombre>.md).
- Alternativa equivalente: .dsh/skills/<nombre>/SKILL.md .
- Skills globales del usuario: ~/.dsh/skills/ .
- Instrucciones de agente: AGENTS.md en la raiz del proyecto (marcada con .git).

## Contenido

| Ruta | Que hay |
|---|---|
| .agents/notes/architecture.md | Flujo de datos, crates, mapas, ciclo de vida del attach |
| .agents/notes/toolchain.md | Versiones exactas, instalacion, por que bpf-linker prebuilt |
| .agents/notes/ebpf-internals.md | connect4, orden de bytes, LPM trie, helpers, reserve_bytes |
| .agents/notes/troubleshooting.md | Catalogo de errores reales: sintoma, causa, arreglo |
| .agents/notes/roadmap.md | Fases 2-4 con tareas concretas |
| .agents/skills/soar-ebpf-agent/SKILL.md | Skill: compilar, probar, extender y depurar el agente |

## Como anadir una skill

1. Crear el directorio .agents/skills/<nombre-kebab>/ .
2. Crear SKILL.md con frontmatter YAML:

    ---
    name: <nombre-kebab>
    description: <cuando usarla, en una frase; incluir palabras clave>
    ---

    <cuerpo con el procedimiento>

3. El nombre debe ser kebab-case y coincidir con el directorio.
4. Se descubre por sesion/workspace; para verla hay que abrir el workspace en este proyecto.

## Como anadir una nota

Un fichero Markdown en .agents/notes/ con el hallazgo, el sintoma y la solucion. Preferir hechos
verificables (comandos y salidas reales) a descripciones vagas.
