module Api
  class ConversionsController < ApplicationController
    skip_before_action :verify_authenticity_token, only: [:convert_url, :convert_file]

    def convert_url
      url = params[:url]

      unless url.present? && url.match?(/\Ahttps?:\/\//i)
        return render json: { error: '유효하지 않은 URL입니다' }, status: :bad_request
      end

      begin
        response = HTTParty.get(url, {
          headers: { 'User-Agent' => 'MDM-Converter/1.0' },
          timeout: 30,
          follow_redirects: true
        })

        html = response.body
        markdown = html_to_markdown(html, url)
        filename = extract_filename_from_url(url)

        render json: {
          markdown: markdown,
          filename: "#{filename}.md",
          source: url,
          converted_at: Time.current.iso8601
        }
      rescue HTTParty::Error, Net::OpenTimeout, SocketError => e
        render json: { error: "URL을 가져올 수 없습니다: #{e.message}" }, status: :unprocessable_entity
      end
    end

    def convert_file
      unless params[:file].present?
        return render json: { error: 'PDF 파일이 필요합니다' }, status: :bad_request
      end

      file = params[:file]

      unless file.content_type == 'application/pdf'
        return render json: { error: 'PDF 파일만 지원합니다' }, status: :bad_request
      end

      begin
        reader = PDF::Reader.new(file.tempfile)
        text_content = []

        reader.pages.each_with_index do |page, index|
          text_content << "## Page #{index + 1}\n\n"
          text_content << page.text.strip
          text_content << "\n\n---\n\n"
        end

        markdown = text_content.join
        filename = File.basename(file.original_filename, '.pdf')

        render json: {
          markdown: markdown,
          filename: "#{filename}.md",
          pages: reader.page_count,
          converted_at: Time.current.iso8601
        }
      rescue PDF::Reader::MalformedPDFError => e
        render json: { error: "PDF 파일이 손상되었습니다: #{e.message}" }, status: :unprocessable_entity
      rescue => e
        render json: { error: "PDF 변환 중 오류 발생: #{e.message}" }, status: :internal_server_error
      end
    end

    private

    def html_to_markdown(html, source_url)
      # Simple HTML to Markdown converter
      doc = html.dup

      # Extract title
      title = doc.match(/<title[^>]*>([^<]*)<\/title>/i)&.[](1) || 'Untitled'

      # Remove script and style tags
      doc.gsub!(/<script[^>]*>[\s\S]*?<\/script>/i, '')
      doc.gsub!(/<style[^>]*>[\s\S]*?<\/style>/i, '')
      doc.gsub!(/<noscript[^>]*>[\s\S]*?<\/noscript>/i, '')

      # Convert headers
      doc.gsub!(/<h1[^>]*>(.*?)<\/h1>/im) { "\n# #{strip_tags($1)}\n" }
      doc.gsub!(/<h2[^>]*>(.*?)<\/h2>/im) { "\n## #{strip_tags($1)}\n" }
      doc.gsub!(/<h3[^>]*>(.*?)<\/h3>/im) { "\n### #{strip_tags($1)}\n" }
      doc.gsub!(/<h4[^>]*>(.*?)<\/h4>/im) { "\n#### #{strip_tags($1)}\n" }
      doc.gsub!(/<h5[^>]*>(.*?)<\/h5>/im) { "\n##### #{strip_tags($1)}\n" }
      doc.gsub!(/<h6[^>]*>(.*?)<\/h6>/im) { "\n###### #{strip_tags($1)}\n" }

      # Convert paragraphs
      doc.gsub!(/<p[^>]*>(.*?)<\/p>/im) { "\n#{strip_tags($1)}\n" }

      # Convert links
      doc.gsub!(/<a[^>]*href=["']([^"']*)["'][^>]*>(.*?)<\/a>/im) { "[#{strip_tags($2)}](#{$1})" }

      # Convert images
      doc.gsub!(/<img[^>]*src=["']([^"']*)["'][^>]*alt=["']([^"']*)["'][^>]*\/?>/im) { "![#{$2}](#{$1})" }
      doc.gsub!(/<img[^>]*src=["']([^"']*)["'][^>]*\/?>/im) { "![image](#{$1})" }

      # Convert lists
      doc.gsub!(/<li[^>]*>(.*?)<\/li>/im) { "- #{strip_tags($1)}" }
      doc.gsub!(/<\/?[uo]l[^>]*>/i, "\n")

      # Convert emphasis
      doc.gsub!(/<(strong|b)[^>]*>(.*?)<\/\1>/im) { "**#{$2}**" }
      doc.gsub!(/<(em|i)[^>]*>(.*?)<\/\1>/im) { "*#{$2}*" }

      # Convert code
      doc.gsub!(/<code[^>]*>(.*?)<\/code>/im) { "`#{$1}`" }
      doc.gsub!(/<pre[^>]*>(.*?)<\/pre>/im) { "\n```\n#{strip_tags($1)}\n```\n" }

      # Convert blockquotes
      doc.gsub!(/<blockquote[^>]*>(.*?)<\/blockquote>/im) { "\n> #{strip_tags($1)}\n" }

      # Convert line breaks
      doc.gsub!(/<br\s*\/?>/i, "\n")
      doc.gsub!(/<hr\s*\/?>/i, "\n---\n")

      # Strip remaining HTML tags
      doc = strip_tags(doc)

      # Clean up whitespace
      doc = doc.gsub(/\n{3,}/, "\n\n").strip

      # Build final markdown
      <<~MARKDOWN
        # #{title}

        > Source: #{source_url}
        > Converted at: #{Time.current.strftime('%Y-%m-%d %H:%M:%S')}

        ---

        #{doc}
      MARKDOWN
    end

    def strip_tags(html)
      html.to_s.gsub(/<[^>]*>/, '').gsub(/&nbsp;/, ' ').gsub(/&amp;/, '&').gsub(/&lt;/, '<').gsub(/&gt;/, '>').gsub(/&quot;/, '"')
    end

    def extract_filename_from_url(url)
      uri = URI.parse(url)
      path = uri.path.gsub(/[^a-zA-Z0-9\-_]/, '_')
      host = uri.host.gsub(/[^a-zA-Z0-9\-_]/, '_')
      "#{host}#{path}".truncate(50, omission: '')
    rescue
      'converted'
    end
  end
end
