#!/bin/bash

ARGUMENTS="$@"

echo "🔍 Analyzing repository and generating task planning..."
TIMESTAMP=$(date +%s)
REPO_FILE="/tmp/repo_${TIMESTAMP}.xml"
TASK_DESCRIPTION="$ARGUMENTS"
echo "📦 Compressing repository with repomix..."
npx repomix@latest -o "$REPO_FILE" --compress . > /dev/null
echo "🤖 Generating comprehensive task planning with Gemini..."
PROMPT="You are an expert technical project manager and software architect. I need comprehensive planning for the following task:

**TASK:** $TASK_DESCRIPTION

**REPOSITORY CONTEXT:** I've provided my complete codebase below using repomix compression. Please analyze the architecture, dependencies, and existing patterns.

**YOUR DELIVERABLES:**
1. **TODO List**: A detailed, prioritized list of implementation steps (use checkboxes format)
2. **Essential Files**: Only the critical file paths needed (existing files to modify + new files to create)  
3. **Architecture Brief**: A comprehensive CLAUDE.md style briefing explaining:
   - How this task fits into the existing architecture
   - Key components and their relationships
   - Implementation strategy and approach
   - Potential risks and considerations
   - Testing strategy

**ANALYSIS REQUIREMENTS:**
- Consider existing code patterns and conventions
- Identify integration points with current systems
- Suggest appropriate error handling and validation
- Recommend testing approaches
- Highlight any architectural changes needed

**REPOSITORY CONTENTS:**
$(cat "$REPO_FILE" 2>/dev/null || echo 'Repository compression failed')

Please provide actionable, specific guidance that another AI assistant can follow to successfully implement this task within the existing codebase architecture."
echo "$PROMPT" | gemini -y -p -
rm -f "$REPO_FILE"
echo "✅ Task planning complete. Follow the guidance above to implement your task."
