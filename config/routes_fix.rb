# 라우트 설정 수정 가이드

# config/routes.rb 파일에 다음 내용을 추가하세요:

Rails.application.routes.draw do
  # ... existing code ...
  
  namespace :api do
    namespace :v1 do
      resources :tasks do
        collection do
          post :generate
        end
      end
    end
    
    # 기존 API 라우트 (v1 없이)
    resources :tasks do
      collection do
        post :generate
      end
    end
  end
  
  # ... existing code ...
end